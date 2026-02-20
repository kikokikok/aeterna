# -----------------------------------------------------------------------------
# GCP Provider Module – Main
# Provisions: GKE Autopilot, Cloud SQL (PostgreSQL HA), Memorystore (Redis HA),
#             GCS, Cloud KMS, Workload Identity
# -----------------------------------------------------------------------------

terraform {
  required_version = ">= 1.6"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
    google-beta = {
      source  = "hashicorp/google-beta"
      version = "~> 6.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
  }
}

locals {
  region       = var.region
  gcs_location = var.gcs_location != "" ? var.gcs_location : var.region
  labels = merge(var.labels, {
    managed_by  = "opentofu"
    environment = var.environment
    application = "aeterna"
  })
}

# ---------- Random suffix for globally unique names ----------

resource "random_id" "suffix" {
  byte_length = 4
}

# =============================================================================
# Networking
# =============================================================================

resource "google_compute_network" "vpc" {
  count                   = var.create_network ? 1 : 0
  project                 = var.project_id
  name                    = "${var.name_prefix}-vpc"
  auto_create_subnetworks = false
}

resource "google_compute_subnetwork" "subnet" {
  count                    = var.create_network ? 1 : 0
  project                  = var.project_id
  name                     = "${var.name_prefix}-subnet"
  region                   = local.region
  network                  = google_compute_network.vpc[0].id
  ip_cidr_range            = var.subnet_cidr
  private_ip_google_access = true

  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = var.pods_cidr
  }

  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = var.services_cidr
  }
}

locals {
  network_id = var.create_network ? google_compute_network.vpc[0].id : "projects/${var.project_id}/global/networks/${var.network_name}"
  subnet_id  = var.create_network ? google_compute_subnetwork.subnet[0].id : null
}

resource "google_compute_global_address" "private_services" {
  count         = var.create_network ? 1 : 0
  project       = var.project_id
  name          = "${var.name_prefix}-private-svc-range"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 20
  network       = local.network_id
}

resource "google_service_networking_connection" "private_vpc_connection" {
  count                   = var.create_network ? 1 : 0
  network                 = local.network_id
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.private_services[0].name]
}

# =============================================================================
# Cloud KMS – CMEK for data-at-rest
# =============================================================================

resource "google_kms_key_ring" "aeterna" {
  project  = var.project_id
  name     = "${var.name_prefix}-keyring-${random_id.suffix.hex}"
  location = local.region
}

resource "google_kms_crypto_key" "aeterna" {
  name            = "${var.name_prefix}-key"
  key_ring        = google_kms_key_ring.aeterna.id
  rotation_period = var.kms_key_rotation_period
  purpose         = "ENCRYPT_DECRYPT"

  lifecycle {
    prevent_destroy = true
  }
}

# =============================================================================
# GKE Autopilot
# =============================================================================

resource "google_container_cluster" "autopilot" {
  provider = google-beta
  project  = var.project_id
  name     = "${var.name_prefix}-gke"
  location = local.region

  enable_autopilot = true
  resource_labels  = local.labels

  release_channel {
    channel = var.gke_release_channel
  }

  network    = local.network_id
  subnetwork = local.subnet_id

  ip_allocation_policy {
    cluster_secondary_range_name  = "pods"
    services_secondary_range_name = "services"
  }

  private_cluster_config {
    enable_private_nodes    = var.gke_enable_private_nodes
    enable_private_endpoint = false
    master_ipv4_cidr_block  = var.gke_master_ipv4_cidr
  }

  dynamic "master_authorized_networks_config" {
    for_each = length(var.gke_master_authorized_networks) > 0 ? [1] : []
    content {
      dynamic "cidr_blocks" {
        for_each = var.gke_master_authorized_networks
        content {
          cidr_block   = cidr_blocks.value.cidr_block
          display_name = cidr_blocks.value.display_name
        }
      }
    }
  }

  # Workload Identity is enabled by default on Autopilot

  deletion_protection = true
}

# =============================================================================
# Cloud SQL – PostgreSQL HA
# =============================================================================

resource "random_password" "cloudsql" {
  length  = 32
  special = true
}

resource "google_sql_database_instance" "postgres" {
  project             = var.project_id
  name                = "${var.name_prefix}-pg-${random_id.suffix.hex}"
  database_version    = var.cloudsql_postgres_version
  region              = local.region
  deletion_protection = true

  encryption_key_name = google_kms_crypto_key.aeterna.id

  depends_on = [
    google_service_networking_connection.private_vpc_connection,
    google_project_iam_member.sql_kms_binding
  ]

  settings {
    tier              = var.cloudsql_tier
    availability_type = var.cloudsql_ha_enabled ? "REGIONAL" : "ZONAL"
    disk_size         = var.cloudsql_disk_size_gb
    disk_autoresize   = var.cloudsql_disk_autoresize
    disk_type         = "PD_SSD"

    user_labels = local.labels

    ip_configuration {
      ipv4_enabled                                  = false
      private_network                               = local.network_id
      enable_private_path_for_google_cloud_services = true
    }

    backup_configuration {
      enabled                        = var.cloudsql_backup_enabled
      point_in_time_recovery_enabled = var.cloudsql_backup_enabled
      start_time                     = "02:00"
      transaction_log_retention_days = 7

      backup_retention_settings {
        retained_backups = 30
      }
    }

    database_flags {
      name  = "cloudsql.iam_authentication"
      value = "on"
    }

    dynamic "database_flags" {
      for_each = var.cloudsql_database_flags
      content {
        name  = database_flags.value.name
        value = database_flags.value.value
      }
    }

    maintenance_window {
      day          = 7 # Sunday
      hour         = 3
      update_track = "stable"
    }
  }
}

resource "google_sql_database" "aeterna" {
  project  = var.project_id
  instance = google_sql_database_instance.postgres.name
  name     = var.cloudsql_database_name
}

resource "google_sql_user" "aeterna" {
  project  = var.project_id
  instance = google_sql_database_instance.postgres.name
  name     = var.cloudsql_user
  password = random_password.cloudsql.result
}

# =============================================================================
# Memorystore – Redis HA
# =============================================================================

resource "google_redis_instance" "aeterna" {
  project            = var.project_id
  name               = "${var.name_prefix}-redis"
  region             = local.region
  tier               = var.redis_tier
  memory_size_gb     = var.redis_memory_size_gb
  customer_managed_key = google_kms_crypto_key.aeterna.id

  depends_on = [
    google_project_iam_member.redis_kms_binding
  ]
  redis_version      = var.redis_version
  auth_enabled       = var.redis_auth_enabled
  authorized_network = local.network_id

  transit_encryption_mode = var.redis_transit_encryption_mode

  labels = local.labels

  maintenance_policy {
    weekly_maintenance_window {
      day = "SUNDAY"
      start_time {
        hours   = 3
        minutes = 0
      }
    }
  }
}

# =============================================================================
# GCS – Object Storage (CMEK encrypted)
# =============================================================================

resource "google_storage_bucket" "aeterna" {
  project       = var.project_id
  name          = "${var.name_prefix}-storage-${random_id.suffix.hex}"
  location      = local.gcs_location
  storage_class = var.gcs_storage_class
  labels        = local.labels

  uniform_bucket_level_access = true

  versioning {
    enabled = var.gcs_versioning_enabled
  }

  dynamic "lifecycle_rule" {
    for_each = var.gcs_lifecycle_age_days > 0 ? [1] : []
    content {
      condition {
        age                        = var.gcs_lifecycle_age_days
        with_state                 = "ARCHIVED"
        num_newer_versions         = 3
      }
      action {
        type = "Delete"
      }
    }
  }

  encryption {
    default_kms_key_name = google_kms_crypto_key.aeterna.id
  }
}

# =============================================================================
# Workload Identity – GCP SA <-> K8s SA binding
# =============================================================================

resource "google_service_account" "aeterna_workload" {
  project      = var.project_id
  account_id   = "${var.name_prefix}-workload"
  display_name = "Aeterna Workload Identity SA"
}

resource "google_project_iam_member" "workload_cloudsql" {
  project = var.project_id
  role    = "roles/cloudsql.client"
  member  = "serviceAccount:${google_service_account.aeterna_workload.email}"
}

resource "google_project_iam_member" "workload_gcs" {
  project = var.project_id
  role    = "roles/storage.objectAdmin"
  member  = "serviceAccount:${google_service_account.aeterna_workload.email}"
}

resource "google_project_iam_member" "workload_kms" {
  project = var.project_id
  role    = "roles/cloudkms.cryptoKeyEncrypterDecrypter"
  member  = "serviceAccount:${google_service_account.aeterna_workload.email}"
}

resource "google_service_account_iam_member" "workload_identity_binding" {
  service_account_id = google_service_account.aeterna_workload.name
  role               = "roles/iam.workloadIdentityUser"
  member             = "serviceAccount:${var.project_id}.svc.id.goog[${var.aeterna_k8s_namespace}/${var.aeterna_k8s_service_account}]"
}
