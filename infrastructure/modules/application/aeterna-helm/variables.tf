variable "namespace" {
  description = "Kubernetes namespace for the Helm release."
  type        = string
  default     = "aeterna"
}

variable "create_namespace" {
  description = "Whether to create the namespace."
  type        = bool
  default     = true
}

variable "release_name" {
  description = "Helm release name."
  type        = string
  default     = "aeterna"
}

variable "chart_path" {
  description = "Path to the Aeterna Helm chart directory."
  type        = string
  default     = "../../../../charts/aeterna"
}

variable "chart_version" {
  description = "Chart version override. Empty uses the version from Chart.yaml."
  type        = string
  default     = ""
}

variable "timeout" {
  description = "Helm install/upgrade timeout in seconds."
  type        = number
  default     = 600
}

variable "atomic" {
  description = "Roll back on failure."
  type        = bool
  default     = true
}

variable "wait" {
  description = "Wait for all resources to be ready."
  type        = bool
  default     = true
}

variable "values_files" {
  description = "List of additional values YAML file paths to merge."
  type        = list(string)
  default     = []
}

variable "postgresql_host" {
  description = "PostgreSQL host from the provider module."
  type        = string
}

variable "postgresql_port" {
  description = "PostgreSQL port."
  type        = number
  default     = 5432
}

variable "postgresql_database" {
  description = "PostgreSQL database name."
  type        = string
}

variable "postgresql_username" {
  description = "PostgreSQL username."
  type        = string
}

variable "postgresql_password" {
  description = "PostgreSQL password."
  type        = string
  sensitive   = true
}

variable "redis_host" {
  description = "Redis host from the provider module."
  type        = string
}

variable "redis_port" {
  description = "Redis port."
  type        = number
  default     = 6379
}

variable "redis_auth_string" {
  description = "Redis AUTH string/password."
  type        = string
  sensitive   = true
  default     = ""
}

variable "object_storage_bucket" {
  description = "Object storage bucket/container name from the provider module."
  type        = string
}

variable "object_storage_url" {
  description = "Object storage URL/ARN/endpoint from the provider module."
  type        = string
}

variable "workload_identity_annotation" {
  description = "Map of annotations to bind the K8s SA to the cloud provider identity."
  type        = map(string)
  default     = {}
}

variable "replica_count" {
  description = "Number of Aeterna replicas."
  type        = number
  default     = 3
}

variable "opal_replica_count" {
  description = "Number of OPAL Server replicas for HA."
  type        = number
  default     = 3
}

variable "extra_values" {
  description = "Arbitrary extra Helm values as a map (deep-merged last)."
  type        = any
  default     = {}
}
