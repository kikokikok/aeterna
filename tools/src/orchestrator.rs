use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::extensions::{ExtensionError, ExtensionExecutor, ExtensionMessage};
use crate::tools::ToolRegistry;
use mk_core::types::TenantContext;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecution {
    pub name: String,
    pub arguments: Value,
    pub result: Value
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrchestratorResult {
    pub output: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolExecution>
}

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Extension error: {0}")]
    Extension(#[from] ExtensionError),

    #[error("Tool error: {0}")]
    Tool(String)
}

#[derive(Debug, Clone)]
pub struct ToolOrchestrator {
    tool_registry: Arc<ToolRegistry>,
    extension_executor: Option<Arc<ExtensionExecutor>>
}

impl ToolOrchestrator {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            extension_executor: None
        }
    }

    pub fn with_extension_executor(mut self, executor: Arc<ExtensionExecutor>) -> Self {
        self.extension_executor = Some(executor);
        self
    }

    pub async fn process_messages(
        &self,
        ctx: TenantContext,
        session_id: &str,
        messages: Vec<ExtensionMessage>
    ) -> Result<Vec<ExtensionMessage>, OrchestratorError> {
        let mut current = messages;
        if let Some(executor) = &self.extension_executor {
            current = executor
                .on_input_messages(ctx, session_id, self.tool_registry.clone(), current)
                .await?;
            current = apply_prompt_wiring(executor, current);
        }
        Ok(current)
    }

    pub async fn process_plain_text(
        &self,
        ctx: TenantContext,
        session_id: &str,
        text: String
    ) -> Result<String, OrchestratorError> {
        let mut current = text;
        if let Some(executor) = &self.extension_executor {
            current = executor
                .on_plain_text(ctx, session_id, self.tool_registry.clone(), current)
                .await?;
            current = apply_prompt_wiring(
                executor,
                vec![ExtensionMessage {
                    role: "user".to_string(),
                    content: current
                }]
            )
            .into_iter()
            .map(|message| message.content)
            .collect::<Vec<_>>()
            .join("\n");
        }
        Ok(current)
    }

    pub fn prompt_wiring(&self) -> Option<crate::extensions::PromptWiring> {
        self.extension_executor
            .as_ref()
            .map(|executor| executor.prompt_wiring())
    }

    pub async fn route_llm_output(
        &self,
        ctx: TenantContext,
        session_id: &str,
        output: String
    ) -> Result<OrchestratorResult, OrchestratorError> {
        let mut output = output;
        if let Some(executor) = &self.extension_executor {
            output = executor
                .on_llm_output(ctx.clone(), session_id, self.tool_registry.clone(), output)
                .await?;
        }

        let (output, tool_calls) = self
            .extract_tool_calls(ctx.clone(), session_id, &output)
            .await?;

        if let Some(executor) = &self.extension_executor {
            let wiring = executor.prompt_wiring();
            let routed_calls =
                apply_tool_config(tool_calls, &wiring.tool_config, &wiring.sequencing_hints);
            let tool_results = self
                .execute_tools(routed_calls.clone())
                .await
                .map_err(OrchestratorError::Tool)?;
            return Ok(OrchestratorResult {
                output,
                tool_calls: routed_calls,
                tool_results
            });
        }

        let tool_results = self
            .execute_tools(tool_calls.clone())
            .await
            .map_err(OrchestratorError::Tool)?;

        Ok(OrchestratorResult {
            output,
            tool_calls,
            tool_results
        })
    }

    async fn extract_tool_calls(
        &self,
        ctx: TenantContext,
        session_id: &str,
        output: &str
    ) -> Result<(String, Vec<ToolCall>), OrchestratorError> {
        let tags = parse_tags(output);
        if tags.is_empty() {
            return Ok((output.to_string(), Vec::new()));
        }

        let mut updated_output = output.to_string();
        let mut tool_calls = Vec::new();

        for tag in tags.iter().rev() {
            let content = &output[tag.content_start..tag.content_end];
            let updated_content = if let Some(executor) = &self.extension_executor {
                match executor
                    .on_tag(
                        ctx.clone(),
                        session_id,
                        self.tool_registry.clone(),
                        tag.name.clone(),
                        content.to_string()
                    )
                    .await
                {
                    Ok(result) => result,
                    Err(_) => content.to_string()
                }
            } else {
                content.to_string()
            };

            if tag.name == "tool" {
                if let Some(call) = parse_tool_call(tag, &updated_content) {
                    tool_calls.push(call);
                }
            }

            updated_output.replace_range(tag.start..tag.end, &updated_content);
        }

        tool_calls.reverse();
        Ok((updated_output, tool_calls))
    }

    async fn execute_tools(&self, calls: Vec<ToolCall>) -> Result<Vec<ToolExecution>, String> {
        let mut results = Vec::new();
        for call in calls {
            let result = self
                .tool_registry
                .call(&call.name, call.arguments.clone())
                .await
                .map_err(|err| err.to_string())?;
            results.push(ToolExecution {
                name: call.name,
                arguments: call.arguments,
                result
            });
        }
        Ok(results)
    }
}

fn apply_prompt_wiring(
    executor: &ExtensionExecutor,
    messages: Vec<ExtensionMessage>
) -> Vec<ExtensionMessage> {
    let wiring = executor.prompt_wiring();
    if wiring.additions.is_empty() {
        return messages;
    }
    let mut additions = wiring
        .additions
        .into_iter()
        .map(|addition| ExtensionMessage {
            role: addition.role,
            content: addition.content
        })
        .collect::<Vec<_>>();
    additions.extend(messages.into_iter());
    additions
}

fn apply_tool_config(
    mut calls: Vec<ToolCall>,
    config: &crate::extensions::ToolConfig,
    sequencing_hints: &[crate::extensions::ToolSequenceHint]
) -> Vec<ToolCall> {
    for call in calls.iter_mut() {
        if let Some(replacement) = config.overrides.get(&call.name) {
            call.name = replacement.clone();
        }
    }

    calls.retain(|call| !config.disabled_tools.contains(&call.name));

    if !config.suggested_tools.is_empty() {
        let order: HashMap<_, _> = config
            .suggested_tools
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx))
            .collect();
        calls.sort_by_key(|call| order.get(&call.name).cloned().unwrap_or(usize::MAX));
    }

    for hint in sequencing_hints {
        let has_when = calls.iter().any(|call| call.name == hint.when_tool);
        let has_next = calls.iter().any(|call| call.name == hint.suggest_next);
        if has_when && !has_next && !config.disabled_tools.contains(&hint.suggest_next) {
            calls.push(ToolCall {
                name: hint.suggest_next.clone(),
                arguments: json!({})
            });
        }
    }

    calls
}

#[cfg(test)]
impl ToolOrchestrator {
    fn new_without_extensions(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            extension_executor: None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TagSegment {
    name: String,
    attributes: HashMap<String, String>,
    start: usize,
    end: usize,
    content_start: usize,
    content_end: usize
}

fn parse_tags(input: &str) -> Vec<TagSegment> {
    let mut segments = Vec::new();
    let mut stack: Vec<(String, HashMap<String, String>, usize, usize)> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] != b'<' {
            idx += 1;
            continue;
        }
        let close = match input[idx..].find('>') {
            Some(pos) => idx + pos,
            None => break
        };
        let tag_body = &input[idx + 1..close];
        if let Some(rest) = tag_body.strip_prefix('/') {
            let name = rest.trim().to_string();
            if let Some(pos) = stack.iter().rposition(|(n, _, _, _)| n == &name) {
                let (start_name, attrs, start_idx, content_start) = stack.remove(pos);
                segments.push(TagSegment {
                    name: start_name,
                    attributes: attrs,
                    start: start_idx,
                    end: close + 1,
                    content_start,
                    content_end: idx
                });
            }
        } else {
            let (name, attrs) = parse_tag_open(tag_body);
            if !name.is_empty() {
                stack.push((name, attrs, idx, close + 1));
            }
        }
        idx = close + 1;
    }

    segments.sort_by_key(|s| s.start);
    segments
}

fn parse_tag_open(tag_body: &str) -> (String, HashMap<String, String>) {
    let mut parts = tag_body.split_whitespace();
    let name = parts.next().unwrap_or("").trim().to_string();
    let mut attrs = HashMap::new();
    for part in parts {
        if let Some((key, value)) = part.split_once('=') {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            attrs.insert(key.to_string(), value.to_string());
        }
    }
    (name, attrs)
}

fn parse_tool_call(tag: &TagSegment, content: &str) -> Option<ToolCall> {
    let name = tag.attributes.get("name").cloned().unwrap_or_default();
    if name.is_empty() {
        return None;
    }

    let arguments = if content.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str::<Value>(content)
            .unwrap_or_else(|_| serde_json::json!({"input": content}))
    };

    Some(ToolCall { name, arguments })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct EchoTool;

    #[async_trait]
    impl crate::tools::Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn description(&self) -> &str {
            "echo"
        }

        fn input_schema(&self) -> Value {
            serde_json::json!({})
        }

        async fn call(
            &self,
            params: Value
        ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
            Ok(serde_json::json!({"echo": params}))
        }
    }

    struct UpperTag;

    #[async_trait]
    impl crate::extensions::ExtensionCallback for UpperTag {
        async fn on_tag(
            &self,
            _ctx: &mut crate::extensions::ExtensionContext,
            _tag: String,
            content: String
        ) -> Result<String, ExtensionError> {
            Ok(format!("\"{}\"", content.to_uppercase()))
        }
    }

    struct ReplaceOutput;

    #[async_trait]
    impl crate::extensions::ExtensionCallback for ReplaceOutput {
        async fn on_llm_output(
            &self,
            _ctx: &mut crate::extensions::ExtensionContext,
            output: String
        ) -> Result<String, ExtensionError> {
            Ok(format!(
                r#"{output}<tool name="echo">{{"value":"ok"}}</tool>"#
            ))
        }
    }

    fn registry_with_echo() -> Arc<ToolRegistry> {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        Arc::new(registry)
    }

    #[test]
    fn test_parse_tags_with_attributes() {
        let input = "<tool name=\"echo\" arg=\"1\">data</tool>";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "tool");
        assert_eq!(tags[0].attributes.get("name").unwrap(), "echo");
        assert_eq!(tags[0].attributes.get("arg").unwrap(), "1");
    }

    #[tokio::test]
    async fn test_parse_and_execute_tool_call() {
        let registry = registry_with_echo();
        let orchestrator = ToolOrchestrator::new_without_extensions(registry.clone());
        let output = "<tool name=\"echo\">{\"value\":\"ok\"}</tool>".to_string();

        let result = orchestrator
            .route_llm_output(TenantContext::default(), "s", output)
            .await
            .unwrap();

        assert_eq!(result.tool_results.len(), 1);
        assert_eq!(result.tool_results[0].result["echo"]["value"], "ok");
    }

    #[tokio::test]
    async fn test_extension_tool_override() {
        let registry = registry_with_echo();
        let mut extension_registry = crate::extensions::ExtensionRegistry::new();
        let mut config = crate::extensions::ToolConfig::default();
        config
            .overrides
            .insert("alias".to_string(), "echo".to_string());
        extension_registry
            .register_extension(
                crate::extensions::ExtensionRegistration::new(
                    "ext",
                    Arc::new(extension_test_helpers::NoopCallback::default())
                )
                .with_tool_config(config)
            )
            .unwrap();

        let executor = Arc::new(ExtensionExecutor::new(Arc::new(extension_registry)));
        let orchestrator =
            ToolOrchestrator::new(registry.clone()).with_extension_executor(executor);
        let output = "<tool name=\"alias\">{\"value\":\"ok\"}</tool>".to_string();

        let result = orchestrator
            .route_llm_output(TenantContext::default(), "s", output)
            .await
            .unwrap();

        assert_eq!(result.tool_results.len(), 1);
        assert_eq!(result.tool_results[0].name, "echo");
    }

    #[tokio::test]
    async fn test_extension_tag_transformation() {
        let registry = registry_with_echo();
        let mut extension_registry = crate::extensions::ExtensionRegistry::new();
        extension_registry
            .register_extension(crate::extensions::ExtensionRegistration::new(
                "ext",
                Arc::new(UpperTag)
            ))
            .unwrap();

        let executor = Arc::new(ExtensionExecutor::new(Arc::new(extension_registry)));
        let orchestrator =
            ToolOrchestrator::new(registry.clone()).with_extension_executor(executor);
        let output = "<tool name=\"echo\">hello</tool>".to_string();

        let result = orchestrator
            .route_llm_output(TenantContext::default(), "s", output)
            .await
            .unwrap();

        assert_eq!(result.tool_results.len(), 1);
        // UpperTag transforms "hello" to "\"HELLO\"", which parses as JSON string
        // "HELLO" EchoTool returns {"echo": <params>}, so result is {"echo":
        // "HELLO"}
        assert_eq!(result.tool_results[0].result["echo"], "HELLO");
    }

    #[tokio::test]
    async fn test_extension_llm_output_callback() {
        let registry = registry_with_echo();
        let mut extension_registry = crate::extensions::ExtensionRegistry::new();
        extension_registry
            .register_extension(crate::extensions::ExtensionRegistration::new(
                "ext",
                Arc::new(ReplaceOutput)
            ))
            .unwrap();

        let executor = Arc::new(ExtensionExecutor::new(Arc::new(extension_registry)));
        let orchestrator =
            ToolOrchestrator::new(registry.clone()).with_extension_executor(executor);

        let result = orchestrator
            .route_llm_output(TenantContext::default(), "s", "result".to_string())
            .await
            .unwrap();

        assert_eq!(result.tool_results.len(), 1);
        assert_eq!(result.tool_results[0].result["echo"]["value"], "ok");
    }

    #[test]
    fn test_parse_tags_nested() {
        let input = "<a>outer <b>inner</b></a>";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 2);
        // Tags are sorted by start position, so 'a' (starts at 0) comes before 'b'
        // (starts at 10)
        assert_eq!(tags[0].name, "a");
        assert_eq!(tags[1].name, "b");
        assert_eq!(&input[tags[1].content_start..tags[1].content_end], "inner");
    }

    #[test]
    fn test_apply_tool_config_sequence_hint() {
        let calls = vec![ToolCall {
            name: "a".to_string(),
            arguments: serde_json::json!({})
        }];
        let config = crate::extensions::ToolConfig::default();
        let hints = vec![crate::extensions::ToolSequenceHint {
            when_tool: "a".to_string(),
            suggest_next: "b".to_string()
        }];

        let result = apply_tool_config(calls, &config, &hints);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|call| call.name == "b"));
    }

    mod extension_test_helpers {
        use super::*;

        #[derive(Default)]
        pub struct NoopCallback;

        #[async_trait]
        impl crate::extensions::ExtensionCallback for NoopCallback {}
    }
}
