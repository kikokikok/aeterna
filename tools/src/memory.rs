#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryOptimizeParams {
    pub layer: String,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
}

pub struct MemoryOptimizeTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryOptimizeTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Tool for MemoryOptimizeTool {
    fn name(&self) -> &str {
        "memory_optimize"
    }

    fn description(&self) -> &str {
        "Manually trigger a pruning/compression cycle for a specific memory layer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "layer": { "type": "string", "enum": ["agent", "user", "session", "project", "team", "org", "company"] },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryOptimizeParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into()),
        };

        self.memory_manager.optimize_layer(ctx, layer).await?;

        Ok(json!({ "success": true, "message": "Memory optimization complete" }))
    }
}
