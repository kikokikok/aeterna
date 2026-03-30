import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export interface LocalConfig {
  enabled: boolean;
  db_path: string;
  sync_push_interval_ms: number;
  sync_pull_interval_ms: number;
  max_cached_entries: number;
  session_storage_ttl_hours: number;
}

export const DEFAULT_LOCAL_CONFIG: LocalConfig = {
  enabled: true,
  db_path: "~/.aeterna/local.db",
  sync_push_interval_ms: 30000,
  sync_pull_interval_ms: 60000,
  max_cached_entries: 50000,
  session_storage_ttl_hours: 24,
};

type LocalFileValues = Partial<LocalConfig>;

export const parseLocalConfig = (
  env: NodeJS.ProcessEnv = process.env,
  cwd: string = process.cwd()
): LocalConfig => {
  const fileConfig = parseLocalConfigToml(cwd);

  const merged: LocalConfig = {
    enabled: parseBoolean(env.AETERNA_LOCAL_ENABLED) ?? fileConfig.enabled ?? DEFAULT_LOCAL_CONFIG.enabled,
    db_path: env.AETERNA_LOCAL_DB_PATH ?? fileConfig.db_path ?? DEFAULT_LOCAL_CONFIG.db_path,
    sync_push_interval_ms:
      parseInteger(env.AETERNA_LOCAL_SYNC_PUSH_INTERVAL_MS) ??
      fileConfig.sync_push_interval_ms ??
      DEFAULT_LOCAL_CONFIG.sync_push_interval_ms,
    sync_pull_interval_ms:
      parseInteger(env.AETERNA_LOCAL_SYNC_PULL_INTERVAL_MS) ??
      fileConfig.sync_pull_interval_ms ??
      DEFAULT_LOCAL_CONFIG.sync_pull_interval_ms,
    max_cached_entries:
      parseInteger(env.AETERNA_LOCAL_MAX_CACHED_ENTRIES) ??
      fileConfig.max_cached_entries ??
      DEFAULT_LOCAL_CONFIG.max_cached_entries,
    session_storage_ttl_hours:
      parseInteger(env.AETERNA_LOCAL_SESSION_STORAGE_TTL_HOURS) ??
      fileConfig.session_storage_ttl_hours ??
      DEFAULT_LOCAL_CONFIG.session_storage_ttl_hours,
  };

  return {
    ...merged,
    db_path: expandHomePath(merged.db_path),
  };
};

const parseLocalConfigToml = (cwd: string): LocalFileValues => {
  const candidates = [join(cwd, ".aeterna", "config.toml"), join(homedir(), ".aeterna", "config.toml")];

  for (const filePath of candidates) {
    if (!existsSync(filePath)) {
      continue;
    }

    const content = readFileSync(filePath, "utf8");
    const values = parseLocalSection(content);
    return values;
  }

  return {};
};

const parseLocalSection = (toml: string): LocalFileValues => {
  const lines = toml.split(/\r?\n/);
  const values: LocalFileValues = {};
  let inLocalSection = false;

  for (const rawLine of lines) {
    const line = stripComments(rawLine).trim();
    if (line.length === 0) {
      continue;
    }

    if (line.startsWith("[") && line.endsWith("]")) {
      inLocalSection = line === "[local]";
      continue;
    }

    if (!inLocalSection) {
      continue;
    }

    const separatorIndex = line.indexOf("=");
    if (separatorIndex <= 0) {
      continue;
    }

    const key = line.slice(0, separatorIndex).trim();
    const value = line.slice(separatorIndex + 1).trim();

    switch (key) {
      case "enabled": {
        const parsed = parseBoolean(value);
        if (parsed !== undefined) {
          values.enabled = parsed;
        }
        break;
      }
      case "db_path": {
        values.db_path = stripQuotes(value);
        break;
      }
      case "sync_push_interval_ms": {
        const parsed = parseInteger(value);
        if (parsed !== undefined) {
          values.sync_push_interval_ms = parsed;
        }
        break;
      }
      case "sync_pull_interval_ms": {
        const parsed = parseInteger(value);
        if (parsed !== undefined) {
          values.sync_pull_interval_ms = parsed;
        }
        break;
      }
      case "max_cached_entries": {
        const parsed = parseInteger(value);
        if (parsed !== undefined) {
          values.max_cached_entries = parsed;
        }
        break;
      }
      case "session_storage_ttl_hours": {
        const parsed = parseInteger(value);
        if (parsed !== undefined) {
          values.session_storage_ttl_hours = parsed;
        }
        break;
      }
      default:
        break;
    }
  }

  return values;
};

const stripComments = (line: string): string => {
  const quoteAware = line.match(/^(?:[^"'#]|"[^"]*"|'[^']*')*/);
  return quoteAware?.[0] ?? line;
};

const parseBoolean = (value: string | undefined): boolean | undefined => {
  if (value === undefined) {
    return undefined;
  }

  const normalized = stripQuotes(value).trim().toLowerCase();
  if (normalized === "true" || normalized === "1" || normalized === "yes" || normalized === "on") {
    return true;
  }
  if (normalized === "false" || normalized === "0" || normalized === "no" || normalized === "off") {
    return false;
  }
  return undefined;
};

const parseInteger = (value: string | undefined): number | undefined => {
  if (value === undefined) {
    return undefined;
  }

  const parsed = Number.parseInt(stripQuotes(value).trim(), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return undefined;
  }

  return parsed;
};

const stripQuotes = (value: string): string => {
  const trimmed = value.trim();
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
};

const expandHomePath = (value: string): string => {
  if (value === "~") {
    return homedir();
  }
  if (value.startsWith("~/")) {
    return join(homedir(), value.slice(2));
  }
  return value;
};
