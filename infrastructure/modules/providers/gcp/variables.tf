# -----------------------------------------------------------------------------
# GCP Provider Module – Variables
# Provisions: GKE Autopilot, Cloud SQL (PostgreSQL HA), Memorystore (Redis HA), GCS
# -----------------------------------------------------------------------------

# ---------- Global ----------

variable "project_id" {
  description = "GCP project ID."
  type        = string
}

variable "region" {
  description = "GCP region for all regional resources."
  type        = string
  default     = "us-central1"
}

variable "environment" {
  description = "Deployment environment label (e.g. dev, staging, prod)."
  type        = string
  default     = "prod"
}

variable "name_prefix" {
  description = "Prefix applied to all resource names for namespacing."
  type        = string
  default     = "aeterna"
}

variable "labels" {
  description = "Common labels applied to all resources."
  type        = map(string)
  default     = {}
}

# ---------- Networking ----------

variable "network_name" {
  description = "Name of the VPC network. Created if create_network is true."
  type        = string
  default     = "aeterna-vpc"
}

variable "subnet_cidr" {
  description = "Primary CIDR range for the subnet."
  type        = string
  default     = "10.0.0.0/20"
}

variable "pods_cidr" {
  description = "Secondary CIDR range for GKE pods."
  type        = string
  default     = "10.4.0.0/14"
}

variable "services_cidr" {
  description = "Secondary CIDR range for GKE services."
  type        = string
  default     = "10.8.0.0/20"
}

variable "create_network" {
  description = "Whether to create the VPC network or use an existing one."
  type        = bool
  default     = true
}

# ---------- GKE Autopilot ----------

variable "gke_release_channel" {
  description = "GKE release channel: REGULAR, RAPID, or STABLE."
  type        = string
  default     = "REGULAR"
}

variable "gke_master_authorized_networks" {
  description = "CIDR blocks allowed to access the GKE API server."
  type = list(object({
    cidr_block   = string
    display_name = string
  }))
  default = []
}

variable "gke_enable_private_nodes" {
  description = "Whether GKE nodes should have only private IPs."
  type        = bool
  default     = true
}

variable "gke_master_ipv4_cidr" {
  description = "CIDR block for GKE control plane when private cluster is enabled."
  type        = string
  default     = "172.16.0.0/28"
}

# ---------- Cloud SQL (PostgreSQL HA) ----------

variable "cloudsql_tier" {
  description = "Cloud SQL machine tier."
  type        = string
  default     = "db-custom-4-16384"
}

variable "cloudsql_postgres_version" {
  description = "PostgreSQL version for Cloud SQL."
  type        = string
  default     = "POSTGRES_16"
}

variable "cloudsql_ha_enabled" {
  description = "Enable Cloud SQL high-availability (regional) configuration."
  type        = bool
  default     = true
}

variable "cloudsql_disk_size_gb" {
  description = "Disk size in GB for Cloud SQL."
  type        = number
  default     = 50
}

variable "cloudsql_disk_autoresize" {
  description = "Enable automatic disk resize for Cloud SQL."
  type        = bool
  default     = true
}

variable "cloudsql_backup_enabled" {
  description = "Enable automated backups for Cloud SQL."
  type        = bool
  default     = true
}

variable "cloudsql_database_flags" {
  description = "Database flags for Cloud SQL instance."
  type = list(object({
    name  = string
    value = string
  }))
  default = [
    { name = "log_checkpoints", value = "on" },
    { name = "log_connections", value = "on" },
    { name = "log_disconnections", value = "on" },
    { name = "log_lock_waits", value = "on" },
  ]
}

variable "cloudsql_database_name" {
  description = "Name of the default database to create."
  type        = string
  default     = "aeterna"
}

variable "cloudsql_user" {
  description = "Name of the default database user."
  type        = string
  default     = "aeterna"
}

# ---------- Memorystore (Redis HA) ----------

variable "redis_tier" {
  description = "Memorystore Redis tier: BASIC or STANDARD_HA."
  type        = string
  default     = "STANDARD_HA"
}

variable "redis_memory_size_gb" {
  description = "Redis memory size in GB."
  type        = number
  default     = 5
}

variable "redis_version" {
  description = "Redis version for Memorystore."
  type        = string
  default     = "REDIS_7_2"
}

variable "redis_auth_enabled" {
  description = "Enable AUTH on Memorystore Redis."
  type        = bool
  default     = true
}

variable "redis_transit_encryption_mode" {
  description = "Transit encryption mode: DISABLED or SERVER_AUTHENTICATION."
  type        = string
  default     = "SERVER_AUTHENTICATION"
}

# ---------- GCS ----------

variable "gcs_location" {
  description = "GCS bucket location. Defaults to var.region."
  type        = string
  default     = ""
}

variable "gcs_storage_class" {
  description = "GCS bucket storage class."
  type        = string
  default     = "STANDARD"
}

variable "gcs_versioning_enabled" {
  description = "Enable object versioning on the GCS bucket."
  type        = bool
  default     = true
}

variable "gcs_lifecycle_age_days" {
  description = "Days after which non-current objects are deleted. 0 disables lifecycle."
  type        = number
  default     = 90
}

# ---------- KMS ----------

variable "kms_key_rotation_period" {
  description = "Rotation period for the Cloud KMS crypto key (e.g. 7776000s = 90 days)."
  type        = string
  default     = "7776000s"
}

# ---------- Workload Identity ----------

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
