use duckdb::{Connection, params};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),
    #[error("Digest mismatch: local={local}, remote={remote}")]
    DigestMismatch { local: String, remote: String },
}

pub fn compute_digest(conn: &Connection, tenant_id: &str) -> Result<[u8; 32], VerifyError> {
    let mut hasher = Sha256::new();

    let mut stmt = conn.prepare(
        "SELECT id, label, properties FROM memory_nodes WHERE tenant_id = ? AND deleted_at IS NULL ORDER BY id",
    )?;
    let mut rows = stmt.query(params![tenant_id])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let label: String = row.get(1)?;
        let props: String = row.get(2)?;
        hasher.update(id.as_bytes());
        hasher.update(label.as_bytes());
        hasher.update(props.as_bytes());
    }

    let mut stmt = conn.prepare(
        "SELECT id, source_id, target_id, relation, properties FROM memory_edges WHERE tenant_id = ? AND deleted_at IS NULL ORDER BY id",
    )?;
    let mut rows = stmt.query(params![tenant_id])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let source: String = row.get(1)?;
        let target: String = row.get(2)?;
        let relation: String = row.get(3)?;
        let props: String = row.get(4)?;
        hasher.update(id.as_bytes());
        hasher.update(source.as_bytes());
        hasher.update(target.as_bytes());
        hasher.update(relation.as_bytes());
        hasher.update(props.as_bytes());
    }

    Ok(hasher.finalize().into())
}

pub fn compute_digest_hex(conn: &Connection, tenant_id: &str) -> Result<String, VerifyError> {
    let digest = compute_digest(conn, tenant_id)?;
    Ok(hex::encode(digest))
}

pub fn verify_digests_match(local: &str, remote: &str) -> Result<(), VerifyError> {
    if local != remote {
        return Err(VerifyError::DigestMismatch {
            local: local.to_string(),
            remote: remote.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE memory_nodes (
                id VARCHAR PRIMARY KEY,
                label VARCHAR NOT NULL,
                properties VARCHAR DEFAULT '{}',
                tenant_id VARCHAR NOT NULL,
                seq BIGINT DEFAULT 0,
                created_at TIMESTAMP DEFAULT now(),
                updated_at TIMESTAMP DEFAULT now(),
                deleted_at TIMESTAMP
            );
            CREATE TABLE memory_edges (
                id VARCHAR PRIMARY KEY,
                source_id VARCHAR NOT NULL,
                target_id VARCHAR NOT NULL,
                relation VARCHAR NOT NULL,
                properties VARCHAR DEFAULT '{}',
                tenant_id VARCHAR NOT NULL,
                seq BIGINT DEFAULT 0,
                created_at TIMESTAMP DEFAULT now(),
                deleted_at TIMESTAMP
            );
            "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_empty_graph_digest_is_deterministic() {
        let conn = setup_test_db();
        let d1 = compute_digest_hex(&conn, "t1").unwrap();
        let d2 = compute_digest_hex(&conn, "t1").unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_same_data_same_digest() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES ('n1', 'label1', '{}', 't1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO memory_edges (id, source_id, target_id, relation, properties, tenant_id) VALUES ('e1', 'n1', 'n1', 'self', '{}', 't1')",
            [],
        ).unwrap();

        let d1 = compute_digest_hex(&conn, "t1").unwrap();
        let d2 = compute_digest_hex(&conn, "t1").unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_different_data_different_digest() {
        let conn = setup_test_db();
        let d_empty = compute_digest_hex(&conn, "t1").unwrap();

        conn.execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES ('n1', 'label1', '{}', 't1')",
            [],
        ).unwrap();
        let d_with_node = compute_digest_hex(&conn, "t1").unwrap();

        assert_ne!(d_empty, d_with_node);
    }

    #[test]
    fn test_deleted_nodes_excluded() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id, deleted_at) VALUES ('n1', 'label1', '{}', 't1', '2025-01-01')",
            [],
        ).unwrap();

        let d = compute_digest_hex(&conn, "t1").unwrap();
        let d_empty = {
            let conn2 = setup_test_db();
            compute_digest_hex(&conn2, "t1").unwrap()
        };
        assert_eq!(d, d_empty);
    }

    #[test]
    fn test_tenant_isolation() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES ('n1', 'label1', '{}', 't1')",
            [],
        ).unwrap();

        let d_t1 = compute_digest_hex(&conn, "t1").unwrap();
        let d_t2 = compute_digest_hex(&conn, "t2").unwrap();
        assert_ne!(d_t1, d_t2);
    }

    #[test]
    fn test_verify_match() {
        assert!(verify_digests_match("abc", "abc").is_ok());
        assert!(verify_digests_match("abc", "def").is_err());
    }
}
