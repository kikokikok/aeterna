output "kubernetes_cluster_name" {
  description = "EKS cluster name."
  value       = aws_eks_cluster.main.name
}

output "kubernetes_cluster_endpoint" {
  description = "EKS cluster API endpoint."
  value       = aws_eks_cluster.main.endpoint
  sensitive   = true
}

output "kubernetes_cluster_ca_certificate" {
  description = "Base64-encoded CA certificate of the EKS cluster."
  value       = aws_eks_cluster.main.certificate_authority[0].data
  sensitive   = true
}

output "kubernetes_cluster_location" {
  description = "AWS region of the EKS cluster."
  value       = var.region
}

output "postgresql_host" {
  description = "RDS PostgreSQL endpoint address."
  value       = aws_db_instance.postgres.address
}

output "postgresql_port" {
  description = "RDS PostgreSQL port."
  value       = aws_db_instance.postgres.port
}

output "postgresql_database" {
  description = "Database name."
  value       = aws_db_instance.postgres.db_name
}

output "postgresql_username" {
  description = "Database master username."
  value       = aws_db_instance.postgres.username
}

output "postgresql_password" {
  description = "Database master password."
  value       = random_password.rds.result
  sensitive   = true
}

output "redis_host" {
  description = "ElastiCache Redis primary endpoint."
  value       = aws_elasticache_replication_group.aeterna.primary_endpoint_address
}

output "redis_port" {
  description = "ElastiCache Redis port."
  value       = 6379
}

output "redis_auth_string" {
  description = "ElastiCache Redis AUTH token (empty when transit encryption handles auth)."
  value       = ""
  sensitive   = true
}

output "object_storage_bucket" {
  description = "S3 bucket name."
  value       = aws_s3_bucket.aeterna.bucket
}

output "object_storage_url" {
  description = "S3 bucket ARN."
  value       = aws_s3_bucket.aeterna.arn
}

output "kms_key_id" {
  description = "KMS key ARN."
  value       = aws_kms_key.aeterna.arn
}

output "workload_identity_sa_email" {
  description = "IRSA role ARN (analogous to GCP workload identity SA email)."
  value       = aws_iam_role.aeterna_irsa.arn
}

output "workload_identity_annotation" {
  description = "Annotation to add to the K8s service account for IRSA."
  value = {
    "eks.amazonaws.com/role-arn" = aws_iam_role.aeterna_irsa.arn
  }
}

output "network_id" {
  description = "VPC ID."
  value       = local.vpc_id
}

output "subnet_ids" {
  description = "Private subnet IDs."
  value       = local.private_subnet_ids
}
