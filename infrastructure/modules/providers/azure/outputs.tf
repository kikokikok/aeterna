output "kubernetes_cluster_name" {
  description = "AKS cluster name."
  value       = azurerm_kubernetes_cluster.main.name
}

output "kubernetes_cluster_endpoint" {
  description = "AKS cluster FQDN."
  value       = azurerm_kubernetes_cluster.main.fqdn
  sensitive   = true
}

output "kubernetes_cluster_ca_certificate" {
  description = "Base64-encoded CA certificate of the AKS cluster."
  value       = azurerm_kubernetes_cluster.main.kube_config[0].cluster_ca_certificate
  sensitive   = true
}

output "kubernetes_cluster_location" {
  description = "Azure region of the AKS cluster."
  value       = azurerm_kubernetes_cluster.main.location
}

output "postgresql_host" {
  description = "Azure DB for PostgreSQL FQDN."
  value       = azurerm_postgresql_flexible_server.main.fqdn
}

output "postgresql_port" {
  description = "PostgreSQL port."
  value       = 5432
}

output "postgresql_database" {
  description = "Database name."
  value       = azurerm_postgresql_flexible_server_database.aeterna.name
}

output "postgresql_username" {
  description = "Database administrator login."
  value       = azurerm_postgresql_flexible_server.main.administrator_login
}

output "postgresql_password" {
  description = "Database administrator password."
  value       = random_password.postgresql.result
  sensitive   = true
}

output "redis_host" {
  description = "Azure Redis Cache hostname."
  value       = azurerm_redis_cache.main.hostname
}

output "redis_port" {
  description = "Azure Redis Cache SSL port."
  value       = azurerm_redis_cache.main.ssl_port
}

output "redis_auth_string" {
  description = "Azure Redis Cache primary access key."
  value       = azurerm_redis_cache.main.primary_access_key
  sensitive   = true
}

output "object_storage_bucket" {
  description = "Azure Storage container name."
  value       = azurerm_storage_container.aeterna.name
}

output "object_storage_url" {
  description = "Azure Storage account primary blob endpoint."
  value       = azurerm_storage_account.main.primary_blob_endpoint
}

output "kms_key_id" {
  description = "Placeholder – use Azure Key Vault for CMEK (configured via AKS secrets provider)."
  value       = ""
}

output "workload_identity_sa_email" {
  description = "Azure Managed Identity client ID."
  value       = azurerm_user_assigned_identity.aeterna.client_id
}

output "workload_identity_annotation" {
  description = "Annotation to add to the K8s service account for Azure Workload Identity."
  value = {
    "azure.workload.identity/client-id" = azurerm_user_assigned_identity.aeterna.client_id
  }
}

output "network_id" {
  description = "VNet ID."
  value       = local.vnet_id
}

output "resource_group_name" {
  description = "Resource group name."
  value       = local.rg_name
}
