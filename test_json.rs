use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ReasoningStrategy {
    Exhaustive,
    Targeted,
    SemanticOnly,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReasoningPlan {
    pub strategy: ReasoningStrategy,
    pub refined_query: String,
    pub reasoning: String,
}

fn main() {
    let plan_json = "{\"strategy\": \"exhaustive\", \"refined_query\": \"deep search for memory-r1 architecture\", \"reasoning\": \"The query is specific and complex, requiring cross-layer verification.\"}";
    let plan: ReasoningPlan = serde_json::from_str(plan_json).expect("Failed to parse JSON");
    println!("{:?}", plan);
}
