output "release_name" {
  description = "Helm release name."
  value       = helm_release.aeterna.name
}

output "release_namespace" {
  description = "Namespace of the Helm release."
  value       = helm_release.aeterna.namespace
}

output "release_revision" {
  description = "Current revision of the Helm release."
  value       = helm_release.aeterna.version
}

output "release_status" {
  description = "Status of the Helm release."
  value       = helm_release.aeterna.status
}

output "postgresql_secret_name" {
  description = "Name of the K8s secret containing PostgreSQL credentials."
  value       = kubernetes_secret.postgresql.metadata[0].name
}

output "redis_secret_name" {
  description = "Name of the K8s secret containing Redis credentials."
  value       = kubernetes_secret.redis.metadata[0].name
}
