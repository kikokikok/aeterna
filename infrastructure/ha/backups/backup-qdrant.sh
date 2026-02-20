#!/usr/bin/env bash
# Qdrant snapshot scheduling — creates snapshots every 6 hours
# Must be run with chmod +x before use
set -euo pipefail

S3_BUCKET="${S3_BUCKET:-s3://aeterna-backups/qdrant}"
QDRANT_URL="${QDRANT_URL:-https://qdrant.aeterna-qdrant.svc.cluster.local:6333}"
QDRANT_API_KEY="${QDRANT_API_KEY:-}"
SNAPSHOT_INTERVAL="${SNAPSHOT_INTERVAL:-21600}"
RETENTION_DAYS="${RETENTION_DAYS:-14}"
TLS_CERT="${TLS_CERT:-/etc/qdrant/tls/client.crt}"
TLS_KEY="${TLS_KEY:-/etc/qdrant/tls/client.key}"
TLS_CA="${TLS_CA:-/etc/qdrant/tls/ca.crt}"
LOG_FILE="/var/log/aeterna/backup-qdrant.log"

log() {
  local level="$1"; shift
  printf '%s [%s] %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$level" "$*" | tee -a "$LOG_FILE"
}

check_deps() {
  for cmd in curl aws jq; do
    if ! command -v "$cmd" &>/dev/null; then
      log ERROR "Required command not found: $cmd"
      exit 1
    fi
  done
}

qdrant_curl() {
  local method="$1"
  local path="$2"
  local extra_args=("${@:3}")

  local auth_args=()
  if [ -n "$QDRANT_API_KEY" ]; then
    auth_args=(--header "api-key: ${QDRANT_API_KEY}")
  fi

  curl -sf \
    --cert "$TLS_CERT" \
    --key "$TLS_KEY" \
    --cacert "$TLS_CA" \
    -X "$method" \
    "${auth_args[@]}" \
    "${extra_args[@]}" \
    "${QDRANT_URL}${path}"
}

list_collections() {
  qdrant_curl GET "/collections" | jq -r '.result.collections[].name'
}

create_snapshot() {
  local collection="$1"
  local timestamp
  timestamp=$(date -u +"%Y%m%d_%H%M%S")

  log INFO "Creating snapshot for collection: $collection"
  local response
  response=$(qdrant_curl POST "/collections/${collection}/snapshots")

  local snapshot_name
  snapshot_name=$(echo "$response" | jq -r '.result.name // empty')
  if [ -z "$snapshot_name" ]; then
    log ERROR "Failed to create snapshot for $collection: $response"
    return 1
  fi

  log INFO "Snapshot created: $snapshot_name"

  local tmp_file="/tmp/qdrant_snapshot_${collection}_${timestamp}.tar"
  log INFO "Downloading snapshot: $snapshot_name"

  if qdrant_curl GET "/collections/${collection}/snapshots/${snapshot_name}" --output "$tmp_file"; then
    log INFO "Snapshot downloaded to $tmp_file"
  else
    log ERROR "Failed to download snapshot: $snapshot_name"
    rm -f "$tmp_file"
    return 1
  fi

  local s3_path="${S3_BUCKET}/${collection}/${timestamp}/${snapshot_name}"
  log INFO "Uploading snapshot to $s3_path"

  if aws s3 cp "$tmp_file" "$s3_path" --sse aws:kms; then
    log INFO "Snapshot uploaded: $s3_path"
  else
    log ERROR "Failed to upload snapshot to S3"
    rm -f "$tmp_file"
    return 1
  fi

  rm -f "$tmp_file"

  qdrant_curl DELETE "/collections/${collection}/snapshots/${snapshot_name}" || \
    log WARN "Could not delete local snapshot $snapshot_name (non-critical)"

  log INFO "Snapshot complete for $collection: $s3_path"
}

snapshot_all() {
  local collections
  collections=$(list_collections)

  if [ -z "$collections" ]; then
    log WARN "No collections found"
    return 0
  fi

  local failed=0
  while IFS= read -r collection; do
    if ! create_snapshot "$collection"; then
      log ERROR "Snapshot failed for collection: $collection"
      failed=$((failed + 1))
    fi
  done <<< "$collections"

  if [ "$failed" -gt 0 ]; then
    log ERROR "$failed collection(s) failed to snapshot"
    return 1
  fi

  log INFO "All collection snapshots completed"
}

cleanup_old_snapshots() {
  log INFO "Cleaning up snapshots older than ${RETENTION_DAYS} days"
  local cutoff_date
  cutoff_date=$(date -u -d "-${RETENTION_DAYS} days" +"%Y%m%d" 2>/dev/null || \
                date -u -v "-${RETENTION_DAYS}d" +"%Y%m%d")

  local collections
  collections=$(aws s3 ls "${S3_BUCKET}/" | awk '{print $NF}' | tr -d '/')

  while IFS= read -r collection; do
    [ -z "$collection" ] && continue
    aws s3 ls "${S3_BUCKET}/${collection}/" | while read -r line; do
      local dir_date
      dir_date=$(echo "$line" | awk '{print $NF}' | tr -d '/' | cut -d_ -f1)
      if [[ "$dir_date" < "$cutoff_date" ]]; then
        local dir_name
        dir_name=$(echo "$line" | awk '{print $NF}' | tr -d '/')
        log INFO "Removing old snapshot: ${collection}/${dir_name}"
        aws s3 rm "${S3_BUCKET}/${collection}/${dir_name}/" --recursive
      fi
    done
  done <<< "$collections"

  log INFO "Cleanup complete"
}

run_daemon() {
  log INFO "Starting Qdrant snapshot daemon (interval: ${SNAPSHOT_INTERVAL}s)"
  while true; do
    snapshot_all || log ERROR "Snapshot cycle failed"
    log INFO "Next snapshot in ${SNAPSHOT_INTERVAL}s"
    sleep "$SNAPSHOT_INTERVAL"
  done
}

usage() {
  cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  snapshot    Create snapshots of all collections and upload to S3
  cleanup     Remove snapshots older than RETENTION_DAYS
  daemon      Run continuously, snapshotting every SNAPSHOT_INTERVAL seconds (default: 6h)
EOF
}

main() {
  mkdir -p "$(dirname "$LOG_FILE")"
  check_deps

  case "${1:-}" in
    snapshot)
      snapshot_all
      ;;
    cleanup)
      cleanup_old_snapshots
      ;;
    daemon)
      run_daemon
      ;;
    *)
      usage
      exit 1
      ;;
  esac
}

main "$@"
