resource "google_project_service_identity" "redis" {
  provider = google-beta
  project  = var.project_id
  service  = "redis.googleapis.com"
}

resource "google_project_iam_member" "redis_kms_binding" {
  project = var.project_id
  role    = "roles/cloudkms.cryptoKeyEncrypterDecrypter"
  member  = "serviceAccount:${google_project_service_identity.redis.email}"
}
