# -----------------------------------------------------------------------------
# GCP Provider Module – Outputs
# Aligned with the aeterna-helm application module inputs
# -----------------------------------------------------------------------------

# ---------- Kubernetes / GKE ----------

output "kubernetes_cluster_name" {
  description = "GKE Autopilot cluster name."
  value       = google_container_cluster.autopilot.name
}

output "kubernetes_cluster_endpoint" {
  description = "GKE Autopilot cluster endpoint."
  value       = google_container_cluster.autopilot.endpoint
  sensitive   = true
}

output "kubernetes_cluster_ca_certificate" {
  description = "Base64-encoded CA certificate of the GKE cluster."
  value       = google_container_cluster.autopilot.master_auth[0].cluster_ca_certificate
  sensitive   = true
}

output "kubernetes_cluster_location" {
  description = "GKE cluster location (region)."
  value       = google_container_cluster.autopilot.location
}

# ---------- PostgreSQL (Cloud SQL) ----------

output "postgresql_host" {
  description = "Cloud SQL private IP address."
  value       = google_sql_database_instance.postgres.private_ip_address
}

output "postgresql_port" {
  description = "Cloud SQL port."
  value       = 5432
}

output "postgresql_database" {
  description = "Database name."
  value       = google_sql_database.aeterna.name
}

output "postgresql_username" {
  description = "Database username."
  value       = google_sql_user.aeterna.name
}

output "postgresql_password" {
  description = "Database password."
  value       = random_password.cloudsql.result
  sensitive   = true
}

output "postgresql_connection_name" {
  description = "Cloud SQL instance connection name (project:region:instance)."
  value       = google_sql_database_instance.postgres.connection_name
}

# ---------- Redis (Memorystore) ----------

output "redis_host" {
  description = "Memorystore Redis host."
  value       = google_redis_instance.aeterna.host
}

output "redis_port" {
  description = "Memorystore Redis port."
  value       = google_redis_instance.aeterna.port
}

output "redis_auth_string" {
  description = "Memorystore Redis AUTH string."
  value       = google_redis_instance.aeterna.auth_string
  sensitive   = true
}

# ---------- Object Storage (GCS) ----------

output "object_storage_bucket" {
  description = "GCS bucket name."
  value       = google_storage_bucket.aeterna.name
}

output "object_storage_url" {
  description = "GCS bucket URL."
  value       = google_storage_bucket.aeterna.url
}

# ---------- KMS ----------

output "kms_key_id" {
  description = "Cloud KMS crypto key resource name."
  value       = google_kms_crypto_key.aeterna.id
}

# ---------- Workload Identity ----------

output "workload_identity_sa_email" {
  description = "GCP service account email for Workload Identity."
  value       = google_service_account.aeterna_workload.email
}

output "workload_identity_annotation" {
  description = "Annotation to add to the K8s service account for Workload Identity."
  value = {
    "iam.gke.io/gcp-service-account" = google_service_account.aeterna_workload.email
  }
}

# ---------- Network ----------

output "network_id" {
  description = "VPC network ID."
  value       = local.network_id
}

output "subnet_id" {
  description = "Subnet ID."
  value       = local.subnet_id
}
