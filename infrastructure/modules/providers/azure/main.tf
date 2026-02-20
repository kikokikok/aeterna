terraform {
  required_version = ">= 1.6"

  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 4.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
  }
}

locals {
  common_tags = merge(var.tags, {
    managed_by  = "opentofu"
    environment = var.environment
    application = "aeterna"
  })
  rg_name = var.create_resource_group ? azurerm_resource_group.main[0].name : var.resource_group_name
}

resource "random_id" "suffix" {
  byte_length = 4
}

resource "random_password" "postgresql" {
  length  = 32
  special = true
}

# =============================================================================
# Resource Group
# =============================================================================

resource "azurerm_resource_group" "main" {
  count    = var.create_resource_group ? 1 : 0
  name     = var.resource_group_name
  location = var.location
  tags     = local.common_tags
}

# =============================================================================
# Networking
# =============================================================================

resource "azurerm_virtual_network" "main" {
  count               = var.create_vnet ? 1 : 0
  name                = "${var.name_prefix}-vnet"
  location            = var.location
  resource_group_name = local.rg_name
  address_space       = [var.vnet_cidr]
  tags                = local.common_tags
}

resource "azurerm_subnet" "aks" {
  count                = var.create_vnet ? 1 : 0
  name                 = "${var.name_prefix}-aks-subnet"
  resource_group_name  = local.rg_name
  virtual_network_name = azurerm_virtual_network.main[0].name
  address_prefixes     = [var.aks_subnet_cidr]
}

resource "azurerm_subnet" "db" {
  count                = var.create_vnet ? 1 : 0
  name                 = "${var.name_prefix}-db-subnet"
  resource_group_name  = local.rg_name
  virtual_network_name = azurerm_virtual_network.main[0].name
  address_prefixes     = [var.db_subnet_cidr]

  delegation {
    name = "postgresql-delegation"
    service_delegation {
      name    = "Microsoft.DBforPostgreSQL/flexibleServers"
      actions = ["Microsoft.Network/virtualNetworks/subnets/join/action"]
    }
  }
}

resource "azurerm_subnet" "redis" {
  count                = var.create_vnet ? 1 : 0
  name                 = "${var.name_prefix}-redis-subnet"
  resource_group_name  = local.rg_name
  virtual_network_name = azurerm_virtual_network.main[0].name
  address_prefixes     = [var.redis_subnet_cidr]
}

resource "azurerm_private_dns_zone" "postgresql" {
  count               = var.create_vnet ? 1 : 0
  name                = "${var.name_prefix}.postgres.database.azure.com"
  resource_group_name = local.rg_name
  tags                = local.common_tags
}

resource "azurerm_private_dns_zone_virtual_network_link" "postgresql" {
  count                 = var.create_vnet ? 1 : 0
  name                  = "${var.name_prefix}-pg-dns-link"
  resource_group_name   = local.rg_name
  private_dns_zone_name = azurerm_private_dns_zone.postgresql[0].name
  virtual_network_id    = azurerm_virtual_network.main[0].id
}

locals {
  vnet_id       = var.create_vnet ? azurerm_virtual_network.main[0].id : null
  aks_subnet_id = var.create_vnet ? azurerm_subnet.aks[0].id : null
  db_subnet_id  = var.create_vnet ? azurerm_subnet.db[0].id : null
}

# =============================================================================
# AKS
# =============================================================================

resource "azurerm_kubernetes_cluster" "main" {
  name                = "${var.name_prefix}-aks"
  location            = var.location
  resource_group_name = local.rg_name
  dns_prefix          = var.name_prefix
  kubernetes_version  = var.aks_kubernetes_version
  sku_tier            = var.aks_sku_tier
  tags                = local.common_tags

  default_node_pool {
    name                 = "default"
    vm_size              = var.aks_default_node_pool.vm_size
    min_count            = var.aks_default_node_pool.min_count
    max_count            = var.aks_default_node_pool.max_count
    node_count           = var.aks_default_node_pool.node_count
    os_disk_size_gb      = var.aks_default_node_pool.os_disk_gb
    zones                = var.aks_default_node_pool.zones
    auto_scaling_enabled = true
    vnet_subnet_id       = local.aks_subnet_id
  }

  identity {
    type = "SystemAssigned"
  }

  oidc_issuer_enabled       = true
  workload_identity_enabled = true

  network_profile {
    network_plugin    = "azure"
    network_policy    = "calico"
    load_balancer_sku = "standard"
    service_cidr      = "10.1.0.0/16"
    dns_service_ip    = "10.1.0.10"
  }

  key_vault_secrets_provider {
    secret_rotation_enabled = true
  }
}

# =============================================================================
# Azure DB for PostgreSQL – Flexible Server (HA)
# =============================================================================

resource "azurerm_postgresql_flexible_server" "main" {
  name                          = "${var.name_prefix}-pg-${random_id.suffix.hex}"
  location                      = var.location
  resource_group_name           = local.rg_name
  version                       = var.postgresql_version
  sku_name                      = var.postgresql_sku_name
  storage_mb                    = var.postgresql_storage_mb
  administrator_login           = var.postgresql_admin_login
  administrator_password        = random_password.postgresql.result
  delegated_subnet_id           = local.db_subnet_id
  private_dns_zone_id           = var.create_vnet ? azurerm_private_dns_zone.postgresql[0].id : null
  backup_retention_days         = var.postgresql_backup_retention_days
  geo_redundant_backup_enabled  = var.postgresql_geo_redundant_backup
  public_network_access_enabled = false
  tags                          = local.common_tags

  high_availability {
    mode = var.postgresql_ha_mode
  }

  depends_on = [azurerm_private_dns_zone_virtual_network_link.postgresql]
}

resource "azurerm_postgresql_flexible_server_database" "aeterna" {
  name      = var.postgresql_database_name
  server_id = azurerm_postgresql_flexible_server.main.id
  charset   = "UTF8"
  collation = "en_US.utf8"
}

# =============================================================================
# Azure Cache for Redis (Premium HA)
# =============================================================================

resource "azurerm_redis_cache" "main" {
  name                          = "${var.name_prefix}-redis-${random_id.suffix.hex}"
  location                      = var.location
  resource_group_name           = local.rg_name
  capacity                      = var.redis_capacity
  family                        = var.redis_family
  sku_name                      = var.redis_sku_name
  non_ssl_port_enabled          = var.redis_enable_non_ssl_port
  minimum_tls_version           = var.redis_minimum_tls_version
  public_network_access_enabled = false
  tags                          = local.common_tags

  redis_configuration {
    maxmemory_policy = "allkeys-lru"
  }
}

# =============================================================================
# Azure Blob Storage
# =============================================================================

resource "azurerm_storage_account" "main" {
  name                            = "${replace(var.name_prefix, "-", "")}st${random_id.suffix.hex}"
  location                        = var.location
  resource_group_name             = local.rg_name
  account_tier                    = var.blob_account_tier
  account_replication_type        = var.blob_replication_type
  min_tls_version                 = "TLS1_2"
  public_network_access_enabled   = false
  allow_nested_items_to_be_public = false
  tags                            = local.common_tags

  blob_properties {
    versioning_enabled = var.blob_versioning_enabled

    delete_retention_policy {
      days = var.blob_delete_retention_days
    }

    container_delete_retention_policy {
      days = var.blob_delete_retention_days
    }
  }
}

resource "azurerm_storage_container" "aeterna" {
  name                  = "aeterna"
  storage_account_id    = azurerm_storage_account.main.id
  container_access_type = "private"
}

# =============================================================================
# Workload Identity – Federated Credential
# =============================================================================

resource "azurerm_user_assigned_identity" "aeterna" {
  name                = "${var.name_prefix}-workload-id"
  location            = var.location
  resource_group_name = local.rg_name
  tags                = local.common_tags
}

resource "azurerm_federated_identity_credential" "aeterna" {
  name                = "${var.name_prefix}-fed-cred"
  resource_group_name = local.rg_name
  parent_id           = azurerm_user_assigned_identity.aeterna.id
  audience            = ["api://AzureADTokenExchange"]
  issuer              = azurerm_kubernetes_cluster.main.oidc_issuer_url
  subject             = "system:serviceaccount:${var.aeterna_k8s_namespace}:${var.aeterna_k8s_service_account}"
}

resource "azurerm_role_assignment" "storage_blob_contributor" {
  scope                = azurerm_storage_account.main.id
  role_definition_name = "Storage Blob Data Contributor"
  principal_id         = azurerm_user_assigned_identity.aeterna.principal_id
}
