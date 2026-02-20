terraform {
  required_version = ">= 1.6"

  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.15"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.33"
    }
  }
}

resource "kubernetes_namespace" "aeterna" {
  count = var.create_namespace ? 1 : 0

  metadata {
    name = var.namespace
    labels = {
      "app.kubernetes.io/managed-by" = "opentofu"
      "app.kubernetes.io/part-of"    = "aeterna"
    }
  }
}

resource "kubernetes_secret" "postgresql" {
  metadata {
    name      = "${var.release_name}-postgresql-credentials"
    namespace = var.namespace
  }

  data = {
    "postgres-password" = var.postgresql_password
  }

  type = "Opaque"

  depends_on = [kubernetes_namespace.aeterna]
}

resource "kubernetes_secret" "redis" {
  metadata {
    name      = "${var.release_name}-redis-credentials"
    namespace = var.namespace
  }

  data = {
    "redis-password" = var.redis_auth_string
  }

  type = "Opaque"

  depends_on = [kubernetes_namespace.aeterna]
}

resource "helm_release" "aeterna" {
  name             = var.release_name
  namespace        = var.namespace
  chart            = var.chart_path
  version          = var.chart_version != "" ? var.chart_version : null
  timeout          = var.timeout
  atomic           = var.atomic
  wait             = var.wait
  create_namespace = false

  values = concat(
    [for f in var.values_files : file(f)],
    [yamlencode({
      aeterna = {
        replicaCount = var.replica_count
        serviceAccount = {
          annotations = var.workload_identity_annotation
        }
        autoscaling = {
          enabled     = true
          minReplicas = var.replica_count
          maxReplicas = var.replica_count * 3
        }
        pdb = {
          enabled      = true
          minAvailable = max(1, var.replica_count - 1)
        }
      }
      postgresql = {
        bundled = false
        external = {
          host           = var.postgresql_host
          port           = var.postgresql_port
          database       = var.postgresql_database
          username       = var.postgresql_username
          existingSecret = kubernetes_secret.postgresql.metadata[0].name
          secretKey      = "postgres-password"
          sslMode        = "require"
        }
      }
      cnpg = {
        enabled = false
      }
      cache = {
        type = "external"
        dragonfly = {
          enabled = false
        }
        external = {
          enabled        = true
          host           = var.redis_host
          port           = var.redis_port
          existingSecret = kubernetes_secret.redis.metadata[0].name
          secretKey      = "redis-password"
        }
      }
      dragonfly = {
        enabled = false
      }
      opal = {
        enabled = true
        server = {
          replicaCount = var.opal_replica_count
        }
      }
    })],
    var.extra_values != {} ? [yamlencode(var.extra_values)] : [],
  )

  depends_on = [
    kubernetes_namespace.aeterna,
    kubernetes_secret.postgresql,
    kubernetes_secret.redis,
  ]
}
