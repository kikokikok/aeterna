variable "location" {
  description = "Azure region for all resources."
  type        = string
  default     = "eastus"
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

variable "resource_group_name" {
  description = "Name of the resource group. Created if create_resource_group is true."
  type        = string
  default     = "aeterna-rg"
}

variable "create_resource_group" {
  description = "Whether to create the resource group."
  type        = bool
  default     = true
}

variable "vnet_cidr" {
  description = "CIDR block for the VNet."
  type        = string
  default     = "10.0.0.0/16"
}

variable "aks_subnet_cidr" {
  description = "CIDR for AKS node subnet."
  type        = string
  default     = "10.0.0.0/20"
}

variable "db_subnet_cidr" {
  description = "CIDR for database delegated subnet."
  type        = string
  default     = "10.0.16.0/24"
}

variable "redis_subnet_cidr" {
  description = "CIDR for Redis Cache subnet."
  type        = string
  default     = "10.0.17.0/24"
}

variable "create_vnet" {
  description = "Whether to create a new VNet."
  type        = bool
  default     = true
}

variable "aks_kubernetes_version" {
  description = "Kubernetes version for AKS."
  type        = string
  default     = "1.31"
}

variable "aks_sku_tier" {
  description = "AKS SKU tier: Free or Standard."
  type        = string
  default     = "Standard"
}

variable "aks_default_node_pool" {
  description = "Default node pool configuration."
  type = object({
    vm_size    = string
    min_count  = number
    max_count  = number
    node_count = number
    os_disk_gb = number
    zones      = list(string)
  })
  default = {
    vm_size    = "Standard_D4s_v5"
    min_count  = 2
    max_count  = 10
    node_count = 3
    os_disk_gb = 50
    zones      = ["1", "2", "3"]
  }
}

variable "postgresql_sku_name" {
  description = "Azure DB for PostgreSQL Flexible Server SKU."
  type        = string
  default     = "GP_Standard_D4s_v3"
}

variable "postgresql_version" {
  description = "PostgreSQL major version."
  type        = string
  default     = "16"
}

variable "postgresql_ha_mode" {
  description = "HA mode: Disabled, SameZone, or ZoneRedundant."
  type        = string
  default     = "ZoneRedundant"
}

variable "postgresql_storage_mb" {
  description = "Storage in MB for PostgreSQL Flexible Server."
  type        = number
  default     = 65536
}

variable "postgresql_backup_retention_days" {
  description = "Backup retention in days."
  type        = number
  default     = 30
}

variable "postgresql_geo_redundant_backup" {
  description = "Enable geo-redundant backups."
  type        = bool
  default     = true
}

variable "postgresql_database_name" {
  description = "Name of the default database."
  type        = string
  default     = "aeterna"
}

variable "postgresql_admin_login" {
  description = "Administrator login for PostgreSQL."
  type        = string
  default     = "aeterna"
}

variable "redis_capacity" {
  description = "Azure Redis Cache capacity (size of the cache)."
  type        = number
  default     = 2
}

variable "redis_family" {
  description = "Azure Redis Cache family: C (Basic/Standard) or P (Premium)."
  type        = string
  default     = "P"
}

variable "redis_sku_name" {
  description = "Azure Redis Cache SKU: Basic, Standard, or Premium."
  type        = string
  default     = "Premium"
}

variable "redis_enable_non_ssl_port" {
  description = "Enable non-SSL port (6379). Should be false in production."
  type        = bool
  default     = false
}

variable "redis_minimum_tls_version" {
  description = "Minimum TLS version for Redis."
  type        = string
  default     = "1.2"
}

variable "redis_replicas_per_primary" {
  description = "Number of replicas per primary (Premium only)."
  type        = number
  default     = 2
}

variable "blob_account_tier" {
  description = "Azure Storage account tier: Standard or Premium."
  type        = string
  default     = "Standard"
}

variable "blob_replication_type" {
  description = "Storage account replication type: LRS, GRS, RAGRS, ZRS, GZRS, RAGZRS."
  type        = string
  default     = "GRS"
}

variable "blob_versioning_enabled" {
  description = "Enable blob versioning."
  type        = bool
  default     = true
}

variable "blob_delete_retention_days" {
  description = "Days to retain deleted blobs."
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
