#!/usr/bin/env bash
# Redis RDB persistence — daily backup to S3
# Must be run with chmod +x before use
set -euo pipefail

S3_BUCKET="${S3_BUCKET:-s3://aeterna-backups/redis}"
REDIS_HOST="${REDIS_HOST:-redis.aeterna-redis.svc.cluster.local}"
REDIS_PORT="${REDIS_PORT:-6379}"
REDIS_PASSWORD="${REDIS_PASSWORD:-}"
TLS_CERT="${TLS_CERT:-/etc/redis/tls/tls.crt}"
TLS_KEY="${TLS_KEY:-/etc/redis/tls/tls.key}"
TLS_CA="${TLS_CA:-/etc/redis/tls/ca.crt}"
REDIS_DATA_DIR="${REDIS_DATA_DIR:-/data}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
LOG_FILE="/var/log/aeterna/backup-redis.log"

log() {
  local level="$1"; shift
  printf '%s [%s] %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$level" "$*" | tee -a "$LOG_FILE"
}

check_deps() {
  for cmd in redis-cli aws; do
    if ! command -v "$cmd" &>/dev/null; then
      log ERROR "Required command not found: $cmd"
      exit 1
    fi
  done
}

redis_cmd() {
  local auth_args=()
  if [ -n "$REDIS_PASSWORD" ]; then
    auth_args=(-a "$REDIS_PASSWORD" --no-auth-warning)
  fi

  redis-cli \
    --tls \
    --cert "$TLS_CERT" \
    --key "$TLS_KEY" \
    --cacert "$TLS_CA" \
    -h "$REDIS_HOST" \
    -p "$REDIS_PORT" \
    "${auth_args[@]}" \
    "$@"
}

trigger_bgsave() {
  log INFO "Triggering BGSAVE"
  local result
  result=$(redis_cmd BGSAVE)

  if [[ "$result" != *"Background saving started"* ]] && [[ "$result" != *"already in progress"* ]]; then
    log ERROR "BGSAVE failed: $result"
    return 1
  fi

  log INFO "Waiting for BGSAVE to complete"
  local max_wait=300
  local waited=0
  while [ "$waited" -lt "$max_wait" ]; do
    local last_save_status
    last_save_status=$(redis_cmd LASTSAVE)
    local bgsave_status
    bgsave_status=$(redis_cmd INFO persistence | grep -o "rdb_bgsave_in_progress:[0-9]*" | cut -d: -f2)

    if [ "${bgsave_status:-1}" = "0" ]; then
      log INFO "BGSAVE completed (LASTSAVE: $last_save_status)"
      return 0
    fi

    sleep 2
    waited=$((waited + 2))
  done

  log ERROR "BGSAVE timed out after ${max_wait}s"
  return 1
}

backup_rdb() {
  local timestamp
  timestamp=$(date -u +"%Y%m%d_%H%M%S")
  local backup_name="dump_${timestamp}.rdb"
  local rdb_file="${REDIS_DATA_DIR}/dump.rdb"

  if ! trigger_bgsave; then
    return 1
  fi

  if [ ! -f "$rdb_file" ]; then
    log ERROR "RDB file not found: $rdb_file"
    return 1
  fi

  local rdb_size
  rdb_size=$(stat -f%z "$rdb_file" 2>/dev/null || stat -c%s "$rdb_file")
  log INFO "RDB file size: ${rdb_size} bytes"

  if [ "$rdb_size" -lt 100 ]; then
    log ERROR "RDB file suspiciously small (${rdb_size} bytes), aborting"
    return 1
  fi

  local tmp_file="/tmp/${backup_name}"
  cp "$rdb_file" "$tmp_file"

  local s3_path="${S3_BUCKET}/${backup_name}"
  log INFO "Uploading RDB to $s3_path"

  if aws s3 cp "$tmp_file" "$s3_path" --sse aws:kms \
    --metadata "timestamp=${timestamp},host=${REDIS_HOST},size=${rdb_size}"; then
    log INFO "RDB uploaded: $s3_path"
  else
    log ERROR "S3 upload failed"
    rm -f "$tmp_file"
    return 1
  fi

  rm -f "$tmp_file"
  log INFO "Redis backup complete: $backup_name"
}

cleanup_old_backups() {
  log INFO "Cleaning up backups older than ${RETENTION_DAYS} days"
  local cutoff_date
  cutoff_date=$(date -u -d "-${RETENTION_DAYS} days" +"%Y%m%d" 2>/dev/null || \
                date -u -v "-${RETENTION_DAYS}d" +"%Y%m%d")

  aws s3 ls "${S3_BUCKET}/" | while read -r line; do
    local filename
    filename=$(echo "$line" | awk '{print $NF}')
    local file_date
    file_date=$(echo "$filename" | sed 's/dump_//' | cut -d_ -f1)
    if [[ "$file_date" < "$cutoff_date" ]]; then
      log INFO "Removing old backup: $filename"
      aws s3 rm "${S3_BUCKET}/${filename}"
    fi
  done

  log INFO "Cleanup complete"
}

verify_backup() {
  local latest
  latest=$(aws s3 ls "${S3_BUCKET}/" | grep "dump_" | sort | tail -1 | awk '{print $NF}')

  if [ -z "$latest" ]; then
    log ERROR "No backups found"
    return 1
  fi

  local size
  size=$(aws s3 ls "${S3_BUCKET}/${latest}" | awk '{print $3}')
  log INFO "Latest backup: $latest (${size} bytes)"

  if [ "${size:-0}" -lt 100 ]; then
    log ERROR "Backup verification failed: file too small"
    return 1
  fi

  log INFO "Backup verification passed"
}

usage() {
  cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  backup     Trigger BGSAVE and upload RDB to S3
  cleanup    Remove backups older than RETENTION_DAYS
  verify     Verify the latest backup
EOF
}

main() {
  mkdir -p "$(dirname "$LOG_FILE")"
  check_deps

  case "${1:-}" in
    backup)
      backup_rdb
      ;;
    cleanup)
      cleanup_old_backups
      ;;
    verify)
      verify_backup
      ;;
    *)
      usage
      exit 1
      ;;
  esac
}

main "$@"
