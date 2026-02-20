variable "region" {
  description = "AWS region for all resources."
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Deployment environment label (e.g. dev, staging, prod)."
  type        = string
  default     = "prod"
}

variable "name_prefix" {
  description = "Prefix applied to all resource names."
  type        = string
  default     = "aeterna"
}

variable "tags" {
  description = "Common tags applied to all resources."
  type        = map(string)
  default     = {}
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC."
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "List of AZs (minimum 2 for HA). Defaults to first 3 in the region."
  type        = list(string)
  default     = []
}

variable "create_vpc" {
  description = "Whether to create a new VPC or use existing subnet IDs."
  type        = bool
  default     = true
}

variable "existing_private_subnet_ids" {
  description = "Private subnet IDs when create_vpc is false."
  type        = list(string)
  default     = []
}

variable "eks_cluster_version" {
  description = "Kubernetes version for EKS."
  type        = string
  default     = "1.31"
}

variable "eks_endpoint_private_access" {
  description = "Enable private API server endpoint."
  type        = bool
  default     = true
}

variable "eks_endpoint_public_access" {
  description = "Enable public API server endpoint."
  type        = bool
  default     = true
}

variable "eks_public_access_cidrs" {
  description = "CIDR blocks allowed to access the public EKS API endpoint."
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "eks_managed_node_group" {
  description = "Managed node group configuration."
  type = object({
    instance_types = list(string)
    min_size       = number
    max_size       = number
    desired_size   = number
    disk_size      = number
  })
  default = {
    instance_types = ["m6i.xlarge"]
    min_size       = 2
    max_size       = 10
    desired_size   = 3
    disk_size      = 50
  }
}

variable "rds_instance_class" {
  description = "RDS instance class."
  type        = string
  default     = "db.r6g.xlarge"
}

variable "rds_engine_version" {
  description = "PostgreSQL engine version for RDS."
  type        = string
  default     = "16.4"
}

variable "rds_multi_az" {
  description = "Enable Multi-AZ for RDS."
  type        = bool
  default     = true
}

variable "rds_allocated_storage" {
  description = "Allocated storage in GB for RDS."
  type        = number
  default     = 50
}

variable "rds_max_allocated_storage" {
  description = "Maximum storage autoscaling limit in GB."
  type        = number
  default     = 200
}

variable "rds_backup_retention_period" {
  description = "Number of days to retain RDS automated backups."
  type        = number
  default     = 30
}

variable "rds_database_name" {
  description = "Name of the default database."
  type        = string
  default     = "aeterna"
}

variable "rds_username" {
  description = "Master username for RDS."
  type        = string
  default     = "aeterna"
}

variable "elasticache_node_type" {
  description = "ElastiCache Redis node type."
  type        = string
  default     = "cache.r6g.large"
}

variable "elasticache_num_cache_clusters" {
  description = "Number of cache clusters (nodes) in the replication group."
  type        = number
  default     = 3
}

variable "elasticache_engine_version" {
  description = "Redis engine version for ElastiCache."
  type        = string
  default     = "7.1"
}

variable "elasticache_at_rest_encryption" {
  description = "Enable encryption at rest for ElastiCache."
  type        = bool
  default     = true
}

variable "elasticache_transit_encryption" {
  description = "Enable in-transit encryption for ElastiCache."
  type        = bool
  default     = true
}

variable "s3_versioning_enabled" {
  description = "Enable S3 bucket versioning."
  type        = bool
  default     = true
}

variable "s3_lifecycle_noncurrent_days" {
  description = "Days after which non-current S3 objects are deleted. 0 disables."
  type        = number
  default     = 90
}

variable "kms_deletion_window_in_days" {
  description = "KMS key deletion waiting period in days."
  type        = number
  default     = 30
}

variable "aeterna_k8s_namespace" {
  description = "Kubernetes namespace where Aeterna workloads run."
  type        = string
  default     = "aeterna"
}

variable "aeterna_k8s_service_account" {
  description = "Kubernetes service account name for Aeterna."
  type        = string
  default     = "aeterna"
}
