/// Robust JSON extraction utilities for parsing LLM responses
///
/// Handles common edge cases in LLM output parsing:
/// - `<think>` tags and other wrapper tags
/// - Multiple JSON objects in the response
/// - Markdown code blocks
/// - String escaping and nested structures

/// Extract all valid JSON objects from a response string.
///
/// Uses brace-depth tracking to robustly identify JSON object boundaries,
/// handling string literals and escape sequences correctly.
///
/// Returns candidates in reverse order (last JSON object preferred,
/// as it's typically the actual response vs. examples or chain-of-thought).
///
/// # Example
/// ```
/// let response = "Some text {\"key\": \"value\"} more text";
/// let candidates = extract_json_candidates(response);
/// assert_eq!(candidates.len(), 1);
/// assert_eq!(candidates[0], r#"{"key": "value"}"#);
/// ```
pub fn extract_json_candidates(response: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in response.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }

                depth -= 1;
                if depth == 0
                    && let Some(start_idx) = start.take()
                {
                    candidates.push(response[start_idx..=idx].to_string());
                }
            }
            _ => {}
        }
    }

    candidates.reverse();
    candidates
}

/// Extract JSON candidates, prioritizing the region after `</think>` tags.
///
/// LLMs using chain-of-thought often wrap reasoning in `<think>` tags.
/// This function:
/// 1. Looks for `</think>` and extracts candidates from the tail
/// 2. Falls back to the full response if no `</think>` found
///
/// # Example
/// ```
/// let response = "<think>reasoning here</think>{\"result\": true}";
/// let candidates = extract_json_candidates_with_think(response);
/// // Candidates will prioritize the region after </think>
/// ```
pub fn extract_json_candidates_with_think(response: &str) -> Vec<String> {
    let candidate_regions = [
        response
            .rsplit_once("</think>")
            .map(|(_, tail)| tail)
            .unwrap_or(response),
        response,
    ];

    let mut all_candidates = Vec::new();
    for region in candidate_regions {
        all_candidates.extend(extract_json_candidates(region));
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    all_candidates.retain(|c| seen.insert(c.clone()));
    all_candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_json() {
        let response = r#"Some text {"key": "value"} more"#;
        let candidates = extract_json_candidates(response);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_multiple_json_prefers_last() {
        let response = r#"{"first": 1} and {"second": 2}"#;
        let candidates = extract_json_candidates(response);
        assert_eq!(candidates.len(), 2);
        // Reversed, so last is first
        assert_eq!(candidates[0], r#"{"second": 2}"#);
        assert_eq!(candidates[1], r#"{"first": 1}"#);
    }

    #[test]
    fn test_extract_nested_objects() {
        let response = r#"{"outer": {"inner": "value"}}"#;
        let candidates = extract_json_candidates(response);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], r#"{"outer": {"inner": "value"}}"#);
    }

    #[test]
    fn test_extract_with_escaped_quotes() {
        let response = r#"{"message": "He said \"hello\""}"#;
        let candidates = extract_json_candidates(response);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], r#"{"message": "He said \"hello\""}"#);
    }

    #[test]
    fn test_extract_with_think_tags() {
        let response = r#"<think>This is reasoning</think>{"result": true}"#;
        let candidates = extract_json_candidates_with_think(response);
        assert!(candidates.contains(&r#"{"result": true}"#.to_string()));
    }

    #[test]
    fn test_extract_no_json_returns_empty() {
        let response = "No JSON here at all";
        let candidates = extract_json_candidates(response);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_extract_unmatched_braces() {
        let response = r#"{"open": {nested"#;
        let candidates = extract_json_candidates(response);
        assert!(candidates.is_empty());
    }
}
