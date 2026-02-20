#!/usr/bin/env bash
# PostgreSQL WAL archiving and base backup to S3
# Must be run with chmod +x before use
set -euo pipefail

S3_BUCKET="${S3_BUCKET:-s3://aeterna-backups/postgres}"
PG_HOST="${PG_HOST:-localhost}"
PG_PORT="${PG_PORT:-5432}"
PG_USER="${PG_USER:-postgres}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
LOG_FILE="/var/log/aeterna/backup-postgres.log"

log() {
  local level="$1"; shift
  printf '%s [%s] %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$level" "$*" | tee -a "$LOG_FILE"
}

check_deps() {
  for cmd in pg_basebackup aws psql; do
    if ! command -v "$cmd" &>/dev/null; then
      log ERROR "Required command not found: $cmd"
      exit 1
    fi
  done
}

archive_wal() {
  local wal_path="$1"
  local wal_name="$2"

  log INFO "Archiving WAL segment: $wal_name"
  if aws s3 cp "$wal_path" "${S3_BUCKET}/wal/${wal_name}" --sse aws:kms; then
    log INFO "WAL segment archived: $wal_name"
  else
    log ERROR "Failed to archive WAL segment: $wal_name"
    exit 1
  fi
}

base_backup() {
  local timestamp
  timestamp=$(date -u +"%Y%m%d_%H%M%S")
  local backup_dir="/tmp/pg_backup_${timestamp}"
  local backup_name="base_${timestamp}"

  log INFO "Starting base backup: $backup_name"
  mkdir -p "$backup_dir"

  if pg_basebackup \
    -h "$PG_HOST" \
    -p "$PG_PORT" \
    -U "$PG_USER" \
    -D "$backup_dir" \
    -Ft \
    -z \
    -Xs \
    --checkpoint=fast \
    --label="$backup_name" \
    --no-password; then
    log INFO "Base backup completed locally"
  else
    log ERROR "pg_basebackup failed"
    rm -rf "$backup_dir"
    exit 1
  fi

  log INFO "Uploading base backup to S3"
  if aws s3 cp "$backup_dir" "${S3_BUCKET}/base/${backup_name}/" --recursive --sse aws:kms; then
    log INFO "Base backup uploaded: ${S3_BUCKET}/base/${backup_name}/"
  else
    log ERROR "S3 upload failed"
    rm -rf "$backup_dir"
    exit 1
  fi

  rm -rf "$backup_dir"

  aws s3api put-object \
    --bucket "$(echo "$S3_BUCKET" | sed 's|s3://||;s|/.*||')" \
    --key "$(echo "$S3_BUCKET" | sed 's|s3://[^/]*/||')/base/${backup_name}/metadata.json" \
    --body <(printf '{"timestamp":"%s","type":"base","host":"%s","retention_days":%d}' \
      "$timestamp" "$PG_HOST" "$RETENTION_DAYS") \
    --sse aws:kms

  log INFO "Base backup complete: $backup_name"
}

cleanup_old_backups() {
  log INFO "Cleaning up backups older than ${RETENTION_DAYS} days"
  local cutoff_date
  cutoff_date=$(date -u -d "-${RETENTION_DAYS} days" +"%Y%m%d" 2>/dev/null || \
                date -u -v "-${RETENTION_DAYS}d" +"%Y%m%d")

  aws s3 ls "${S3_BUCKET}/base/" | while read -r line; do
    local dir_name
    dir_name=$(echo "$line" | awk '{print $NF}' | tr -d '/')
    local dir_date
    dir_date=$(echo "$dir_name" | sed 's/base_//' | cut -d_ -f1)
    if [[ "$dir_date" < "$cutoff_date" ]]; then
      log INFO "Removing old backup: $dir_name"
      aws s3 rm "${S3_BUCKET}/base/${dir_name}/" --recursive
    fi
  done

  log INFO "Cleanup complete"
}

verify_backup() {
  local latest
  latest=$(aws s3 ls "${S3_BUCKET}/base/" | sort | tail -1 | awk '{print $NF}' | tr -d '/')

  if [ -z "$latest" ]; then
    log ERROR "No backups found to verify"
    return 1
  fi

  log INFO "Verifying latest backup: $latest"
  local file_count
  file_count=$(aws s3 ls "${S3_BUCKET}/base/${latest}/" --recursive | wc -l)

  if [ "$file_count" -lt 2 ]; then
    log ERROR "Backup verification failed: insufficient files ($file_count)"
    return 1
  fi

  log INFO "Backup verification passed: $file_count files found"
}

usage() {
  cat <<EOF
Usage: $(basename "$0") <command> [args]

Commands:
  wal <path> <name>   Archive a WAL segment to S3
  base                Perform a full base backup
  cleanup             Remove backups older than RETENTION_DAYS
  verify              Verify the latest backup integrity
EOF
}

main() {
  mkdir -p "$(dirname "$LOG_FILE")"
  check_deps

  case "${1:-}" in
    wal)
      [ $# -lt 3 ] && { log ERROR "Usage: $0 wal <path> <name>"; exit 1; }
      archive_wal "$2" "$3"
      ;;
    base)
      base_backup
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
