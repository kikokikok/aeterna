pub fn record_node_count(tenant_id: &str, count: usize) {
    metrics::gauge!("graph_nodes_total", "tenant_id" => tenant_id.to_string()).set(count as f64);
}

pub fn record_edge_count(tenant_id: &str, count: usize) {
    metrics::gauge!("graph_edges_total", "tenant_id" => tenant_id.to_string()).set(count as f64);
}

pub fn record_traversal_depth(depth: usize) {
    metrics::histogram!("graph_traversal_depth").record(depth as f64);
}

pub fn record_lock_wait_ms(ms: f64) {
    metrics::histogram!("graph_duckdb_lock_wait_ms").record(ms);
}

pub fn record_partition_load_ms(ms: f64) {
    metrics::histogram!("graph_partition_load_ms").record(ms);
}

pub fn record_snapshot_bytes(bytes: u64) {
    metrics::histogram!("graph_snapshot_bytes").record(bytes as f64);
}
