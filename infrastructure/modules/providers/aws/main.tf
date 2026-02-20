terraform {
  required_version = ">= 1.6"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4.0"
    }
  }
}

data "aws_availability_zones" "available" {
  state = "available"
}

locals {
  azs = length(var.availability_zones) > 0 ? var.availability_zones : slice(data.aws_availability_zones.available.names, 0, 3)
  common_tags = merge(var.tags, {
    managed_by  = "opentofu"
    environment = var.environment
    application = "aeterna"
  })
}

resource "random_id" "suffix" {
  byte_length = 4
}

resource "random_password" "rds" {
  length  = 32
  special = true
}

# =============================================================================
# VPC
# =============================================================================

resource "aws_vpc" "main" {
  count                = var.create_vpc ? 1 : 0
  cidr_block           = var.vpc_cidr
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = merge(local.common_tags, { Name = "${var.name_prefix}-vpc" })
}

resource "aws_subnet" "private" {
  count             = var.create_vpc ? length(local.azs) : 0
  vpc_id            = aws_vpc.main[0].id
  cidr_block        = cidrsubnet(var.vpc_cidr, 4, count.index)
  availability_zone = local.azs[count.index]

  tags = merge(local.common_tags, {
    Name                                           = "${var.name_prefix}-private-${local.azs[count.index]}"
    "kubernetes.io/role/internal-elb"              = "1"
    "kubernetes.io/cluster/${var.name_prefix}-eks" = "shared"
  })
}

resource "aws_subnet" "public" {
  count                   = var.create_vpc ? length(local.azs) : 0
  vpc_id                  = aws_vpc.main[0].id
  cidr_block              = cidrsubnet(var.vpc_cidr, 4, count.index + length(local.azs))
  availability_zone       = local.azs[count.index]
  map_public_ip_on_launch = true

  tags = merge(local.common_tags, {
    Name                                           = "${var.name_prefix}-public-${local.azs[count.index]}"
    "kubernetes.io/role/elb"                       = "1"
    "kubernetes.io/cluster/${var.name_prefix}-eks" = "shared"
  })
}

resource "aws_internet_gateway" "main" {
  count  = var.create_vpc ? 1 : 0
  vpc_id = aws_vpc.main[0].id
  tags   = merge(local.common_tags, { Name = "${var.name_prefix}-igw" })
}

resource "aws_eip" "nat" {
  count  = var.create_vpc ? 1 : 0
  domain = "vpc"
  tags   = merge(local.common_tags, { Name = "${var.name_prefix}-nat-eip" })
}

resource "aws_nat_gateway" "main" {
  count         = var.create_vpc ? 1 : 0
  allocation_id = aws_eip.nat[0].id
  subnet_id     = aws_subnet.public[0].id
  tags          = merge(local.common_tags, { Name = "${var.name_prefix}-nat" })
}

resource "aws_route_table" "private" {
  count  = var.create_vpc ? 1 : 0
  vpc_id = aws_vpc.main[0].id
  tags   = merge(local.common_tags, { Name = "${var.name_prefix}-private-rt" })
}

resource "aws_route" "private_nat" {
  count                  = var.create_vpc ? 1 : 0
  route_table_id         = aws_route_table.private[0].id
  destination_cidr_block = "0.0.0.0/0"
  nat_gateway_id         = aws_nat_gateway.main[0].id
}

resource "aws_route_table_association" "private" {
  count          = var.create_vpc ? length(local.azs) : 0
  subnet_id      = aws_subnet.private[count.index].id
  route_table_id = aws_route_table.private[0].id
}

resource "aws_route_table" "public" {
  count  = var.create_vpc ? 1 : 0
  vpc_id = aws_vpc.main[0].id
  tags   = merge(local.common_tags, { Name = "${var.name_prefix}-public-rt" })
}

resource "aws_route" "public_igw" {
  count                  = var.create_vpc ? 1 : 0
  route_table_id         = aws_route_table.public[0].id
  destination_cidr_block = "0.0.0.0/0"
  gateway_id             = aws_internet_gateway.main[0].id
}

resource "aws_route_table_association" "public" {
  count          = var.create_vpc ? length(local.azs) : 0
  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public[0].id
}

locals {
  vpc_id             = var.create_vpc ? aws_vpc.main[0].id : null
  private_subnet_ids = var.create_vpc ? aws_subnet.private[*].id : var.existing_private_subnet_ids
}

# =============================================================================
# KMS
# =============================================================================

resource "aws_kms_key" "aeterna" {
  description             = "Aeterna CMEK for data-at-rest encryption"
  deletion_window_in_days = var.kms_deletion_window_in_days
  enable_key_rotation     = true
  tags                    = local.common_tags
}

resource "aws_kms_alias" "aeterna" {
  name          = "alias/${var.name_prefix}-key"
  target_key_id = aws_kms_key.aeterna.key_id
}

# =============================================================================
# EKS
# =============================================================================

resource "aws_iam_role" "eks_cluster" {
  name = "${var.name_prefix}-eks-cluster-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = { Service = "eks.amazonaws.com" }
    }]
  })

  tags = local.common_tags
}

resource "aws_iam_role_policy_attachment" "eks_cluster_policy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSClusterPolicy"
  role       = aws_iam_role.eks_cluster.name
}

resource "aws_iam_role_policy_attachment" "eks_vpc_resource_controller" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSVPCResourceController"
  role       = aws_iam_role.eks_cluster.name
}

resource "aws_security_group" "eks_cluster" {
  name_prefix = "${var.name_prefix}-eks-cluster-"
  vpc_id      = local.vpc_id
  description = "EKS cluster security group"

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = var.eks_public_access_cidrs
    description = "Kubernetes API server"
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
    description = "All outbound"
  }

  tags = local.common_tags

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_eks_cluster" "main" {
  name     = "${var.name_prefix}-eks"
  version  = var.eks_cluster_version
  role_arn = aws_iam_role.eks_cluster.arn

  vpc_config {
    subnet_ids              = local.private_subnet_ids
    endpoint_private_access = var.eks_endpoint_private_access
    endpoint_public_access  = var.eks_endpoint_public_access
    public_access_cidrs     = var.eks_public_access_cidrs
    security_group_ids      = [aws_security_group.eks_cluster.id]
  }

  encryption_config {
    provider {
      key_arn = aws_kms_key.aeterna.arn
    }
    resources = ["secrets"]
  }

  tags = local.common_tags

  depends_on = [
    aws_iam_role_policy_attachment.eks_cluster_policy,
    aws_iam_role_policy_attachment.eks_vpc_resource_controller,
  ]
}

resource "aws_iam_role" "eks_node_group" {
  name = "${var.name_prefix}-eks-node-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
    }]
  })

  tags = local.common_tags
}

resource "aws_iam_role_policy_attachment" "eks_worker_node_policy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSWorkerNodePolicy"
  role       = aws_iam_role.eks_node_group.name
}

resource "aws_iam_role_policy_attachment" "eks_cni_policy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKS_CNI_Policy"
  role       = aws_iam_role.eks_node_group.name
}

resource "aws_iam_role_policy_attachment" "ecr_read_only" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly"
  role       = aws_iam_role.eks_node_group.name
}

resource "aws_eks_node_group" "main" {
  cluster_name    = aws_eks_cluster.main.name
  node_group_name = "${var.name_prefix}-default"
  node_role_arn   = aws_iam_role.eks_node_group.arn
  subnet_ids      = local.private_subnet_ids
  instance_types  = var.eks_managed_node_group.instance_types
  disk_size       = var.eks_managed_node_group.disk_size

  scaling_config {
    min_size     = var.eks_managed_node_group.min_size
    max_size     = var.eks_managed_node_group.max_size
    desired_size = var.eks_managed_node_group.desired_size
  }

  update_config {
    max_unavailable = 1
  }

  tags = local.common_tags

  depends_on = [
    aws_iam_role_policy_attachment.eks_worker_node_policy,
    aws_iam_role_policy_attachment.eks_cni_policy,
    aws_iam_role_policy_attachment.ecr_read_only,
  ]
}

# =============================================================================
# RDS – PostgreSQL Multi-AZ
# =============================================================================

resource "aws_db_subnet_group" "aeterna" {
  name       = "${var.name_prefix}-db-subnet"
  subnet_ids = local.private_subnet_ids
  tags       = local.common_tags
}

resource "aws_security_group" "rds" {
  name_prefix = "${var.name_prefix}-rds-"
  vpc_id      = local.vpc_id
  description = "RDS PostgreSQL security group"

  ingress {
    from_port       = 5432
    to_port         = 5432
    protocol        = "tcp"
    security_groups = [aws_security_group.eks_cluster.id]
    description     = "PostgreSQL from EKS"
  }

  tags = local.common_tags

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_db_instance" "postgres" {
  identifier     = "${var.name_prefix}-pg-${random_id.suffix.hex}"
  engine         = "postgres"
  engine_version = var.rds_engine_version
  instance_class = var.rds_instance_class

  allocated_storage     = var.rds_allocated_storage
  max_allocated_storage = var.rds_max_allocated_storage
  storage_type          = "gp3"
  storage_encrypted     = true
  kms_key_id            = aws_kms_key.aeterna.arn

  db_name  = var.rds_database_name
  username = var.rds_username
  password = random_password.rds.result

  multi_az               = var.rds_multi_az
  db_subnet_group_name   = aws_db_subnet_group.aeterna.name
  vpc_security_group_ids = [aws_security_group.rds.id]

  backup_retention_period   = var.rds_backup_retention_period
  backup_window             = "02:00-03:00"
  maintenance_window        = "sun:03:00-sun:04:00"
  copy_tags_to_snapshot     = true
  deletion_protection       = true
  skip_final_snapshot       = false
  final_snapshot_identifier = "${var.name_prefix}-pg-final-${random_id.suffix.hex}"

  performance_insights_enabled    = true
  performance_insights_kms_key_id = aws_kms_key.aeterna.arn

  tags = local.common_tags
}

# =============================================================================
# ElastiCache – Redis HA
# =============================================================================

resource "aws_elasticache_subnet_group" "aeterna" {
  name       = "${var.name_prefix}-redis-subnet"
  subnet_ids = local.private_subnet_ids
  tags       = local.common_tags
}

resource "aws_security_group" "elasticache" {
  name_prefix = "${var.name_prefix}-redis-"
  vpc_id      = local.vpc_id
  description = "ElastiCache Redis security group"

  ingress {
    from_port       = 6379
    to_port         = 6379
    protocol        = "tcp"
    security_groups = [aws_security_group.eks_cluster.id]
    description     = "Redis from EKS"
  }

  tags = local.common_tags

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_elasticache_replication_group" "aeterna" {
  replication_group_id = "${var.name_prefix}-redis"
  description          = "Aeterna Redis HA replication group"
  node_type            = var.elasticache_node_type
  num_cache_clusters   = var.elasticache_num_cache_clusters
  engine_version       = var.elasticache_engine_version

  subnet_group_name  = aws_elasticache_subnet_group.aeterna.name
  security_group_ids = [aws_security_group.elasticache.id]

  at_rest_encryption_enabled = var.elasticache_at_rest_encryption
  transit_encryption_enabled = var.elasticache_transit_encryption
  kms_key_id                 = aws_kms_key.aeterna.arn

  automatic_failover_enabled = var.elasticache_num_cache_clusters > 1
  multi_az_enabled           = var.elasticache_num_cache_clusters > 1

  maintenance_window       = "sun:03:00-sun:04:00"
  snapshot_retention_limit = 7
  snapshot_window          = "02:00-03:00"

  tags = local.common_tags
}

# =============================================================================
# S3 – Object Storage (CMEK encrypted)
# =============================================================================

resource "aws_s3_bucket" "aeterna" {
  bucket = "${var.name_prefix}-storage-${random_id.suffix.hex}"
  tags   = local.common_tags
}

resource "aws_s3_bucket_versioning" "aeterna" {
  bucket = aws_s3_bucket.aeterna.id
  versioning_configuration {
    status = var.s3_versioning_enabled ? "Enabled" : "Suspended"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "aeterna" {
  bucket = aws_s3_bucket.aeterna.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = "aws:kms"
      kms_master_key_id = aws_kms_key.aeterna.arn
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_bucket_public_access_block" "aeterna" {
  bucket                  = aws_s3_bucket.aeterna.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_lifecycle_configuration" "aeterna" {
  count  = var.s3_lifecycle_noncurrent_days > 0 ? 1 : 0
  bucket = aws_s3_bucket.aeterna.id

  rule {
    id     = "cleanup-noncurrent"
    status = "Enabled"

    noncurrent_version_expiration {
      noncurrent_days = var.s3_lifecycle_noncurrent_days
    }
  }
}

# =============================================================================
# IRSA – IAM Roles for Service Accounts
# =============================================================================

data "tls_certificate" "eks" {
  url = aws_eks_cluster.main.identity[0].oidc[0].issuer
}

resource "aws_iam_openid_connect_provider" "eks" {
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = [data.tls_certificate.eks.certificates[0].sha1_fingerprint]
  url             = aws_eks_cluster.main.identity[0].oidc[0].issuer
  tags            = local.common_tags
}

locals {
  oidc_provider_arn = aws_iam_openid_connect_provider.eks.arn
  oidc_issuer       = replace(aws_eks_cluster.main.identity[0].oidc[0].issuer, "https://", "")
}

resource "aws_iam_role" "aeterna_irsa" {
  name = "${var.name_prefix}-irsa"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Principal = { Federated = local.oidc_provider_arn }
      Action = "sts:AssumeRoleWithWebIdentity"
      Condition = {
        StringEquals = {
          "${local.oidc_issuer}:sub" = "system:serviceaccount:${var.aeterna_k8s_namespace}:${var.aeterna_k8s_service_account}"
          "${local.oidc_issuer}:aud" = "sts.amazonaws.com"
        }
      }
    }]
  })

  tags = local.common_tags
}

resource "aws_iam_role_policy" "aeterna_irsa_s3" {
  name = "${var.name_prefix}-irsa-s3"
  role = aws_iam_role.aeterna_irsa.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"]
      Resource = [aws_s3_bucket.aeterna.arn, "${aws_s3_bucket.aeterna.arn}/*"]
    }]
  })
}

resource "aws_iam_role_policy" "aeterna_irsa_kms" {
  name = "${var.name_prefix}-irsa-kms"
  role = aws_iam_role.aeterna_irsa.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["kms:Decrypt", "kms:Encrypt", "kms:GenerateDataKey"]
      Resource = [aws_kms_key.aeterna.arn]
    }]
  })
}
