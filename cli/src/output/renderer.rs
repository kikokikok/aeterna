//! Output renderer for `aeterna` CLI commands (B2 §9.1 + §9.2).
//!
//! Before this module every command that supported JSON output
//! hand-rolled the same `if args.json { … } else { … }` branch. That
//! pattern (1) only offered two shapes, (2) picked defaults
//! inconsistently per command, and (3) left no single place to add a
//! new format. This module replaces it with a [`Renderer`] driven by
//! a parsed [`OutputFormat`].
//!
//! Default rule (matches `kubectl`, `gh`, `aws`, `terraform`): render
//! a table on a TTY, render JSON when piped. Callers override with
//! `-o json|yaml|name|jsonpath=<expr>`.
//!
//! JSONPath dialect is the kubectl-compatible subset: optional
//! `{ … }` wrapper, `.foo.bar` for object access, `[N]` for numeric
//! array indexing. Wildcards, filters, slices, and recursive descent
//! are explicitly unsupported — pipe to `jq` if you need them.
//!
//! This PR lands the primitives and tests; migrating existing
//! `args.json` call sites is a deliberate follow-up so reviewers can
//! evaluate the API in isolation.

use std::fmt;

use serde::Serialize;
use serde_json::Value;

/// Output shape, post `-o <FORMAT>` parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable table. Layout is caller-defined; [`Renderer`]
    /// falls back to pretty JSON when the value cannot be tabulated.
    Table,
    /// Pretty-printed JSON via `serde_json`.
    Json,
    /// YAML via `serde_yaml`.
    Yaml,
    /// Identifier-only; caller supplies the name field via
    /// [`Renderer::render_with_name`].
    Name,
    /// kubectl-style `jsonpath=<EXPR>` extractor. See module docs for
    /// the supported subset.
    JsonPath(String),
}

impl OutputFormat {
    /// Default when `-o` is absent. TTY → Table, pipe → Json.
    #[must_use]
    pub fn default_for_tty(is_tty: bool) -> Self {
        if is_tty { Self::Table } else { Self::Json }
    }

    /// Parse the user-supplied `-o` value. Case-insensitive for bare
    /// keywords. Everything after the first `=` in `jsonpath=<EXPR>`
    /// is taken verbatim; empty expressions are rejected.
    ///
    /// # Errors
    /// Returns [`ParseFormatError`] for unknown keywords or an empty
    /// `jsonpath=` expression.
    pub fn parse(raw: &str) -> Result<Self, ParseFormatError> {
        let trimmed = raw.trim();
        if let Some(expr) = trimmed.strip_prefix("jsonpath=") {
            if expr.is_empty() {
                return Err(ParseFormatError::EmptyJsonPath);
            }
            return Ok(Self::JsonPath(expr.to_string()));
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            "name" => Ok(Self::Name),
            other => Err(ParseFormatError::Unknown(other.to_string())),
        }
    }

    /// Lowercase label used in help text and error messages.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Table => "table",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Name => "name",
            Self::JsonPath(_) => "jsonpath",
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonPath(expr) => write!(f, "jsonpath={expr}"),
            other => f.write_str(other.label()),
        }
    }
}

/// Flag-parse-time error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseFormatError {
    /// The bare keyword was not one of the four supported values.
    Unknown(String),
    /// `jsonpath=` was supplied with an empty expression.
    EmptyJsonPath,
}

impl fmt::Display for ParseFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(k) => write!(
                f,
                "unknown output format '{k}' (expected one of: table, json, yaml, name, jsonpath=<expr>)"
            ),
            Self::EmptyJsonPath => write!(f, "jsonpath= requires a non-empty expression"),
        }
    }
}

impl std::error::Error for ParseFormatError {}

/// Extract-time error (the expression parsed fine but does not fit
/// the value it was applied to).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPathError {
    /// Field access on something that is not an object.
    NotAnObject { at: String },
    /// Index access on something that is not an array.
    NotAnArray { at: String },
    /// Field does not exist on the object.
    MissingField { field: String },
    /// Index out of bounds for the array.
    IndexOutOfBounds { index: usize, len: usize },
    /// Stray bracket, non-numeric index, etc.
    MalformedExpression(String),
    /// Wildcard, filter, slice, recursive descent.
    UnsupportedConstruct(String),
}

impl fmt::Display for JsonPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject { at } => write!(f, "value at '{at}' is not an object"),
            Self::NotAnArray { at } => write!(f, "value at '{at}' is not an array"),
            Self::MissingField { field } => write!(f, "missing field '{field}'"),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds (length {len})")
            }
            Self::MalformedExpression(e) => write!(f, "malformed jsonpath expression: {e}"),
            Self::UnsupportedConstruct(e) => write!(f, "unsupported jsonpath construct: {e}"),
        }
    }
}

impl std::error::Error for JsonPathError {}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Turns a serialisable value into the format the user asked for.
///
/// Typically built once per command invocation from the `-o` flag
/// (or [`OutputFormat::default_for_tty`]).
#[derive(Debug, Clone)]
pub struct Renderer {
    format: OutputFormat,
}

impl Renderer {
    /// Construct a renderer for a specific format.
    #[must_use]
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Borrow the format so callers can branch on `Table` before
    /// delegating to [`Self::render_structured`] for everything else.
    #[must_use]
    pub fn format(&self) -> &OutputFormat {
        &self.format
    }

    /// Render a serialisable value.
    ///
    /// - `Json` → pretty JSON
    /// - `Yaml` → YAML
    /// - `JsonPath` → extracted via [`jsonpath_extract`]
    /// - `Table` / `Name` → falls back to pretty JSON because generic
    ///   structured values cannot be tabled without caller-supplied
    ///   column layout; commands that want a real table should branch
    ///   on [`Self::format`] and emit their own output for `Table`.
    ///
    /// # Errors
    /// Serialisation errors are wrapped in [`RenderError::Serialize`];
    /// JSONPath extraction errors in [`RenderError::JsonPath`].
    pub fn render_structured<T: Serialize>(&self, value: &T) -> Result<String, RenderError> {
        match &self.format {
            OutputFormat::Json | OutputFormat::Table | OutputFormat::Name => {
                serde_json::to_string_pretty(value).map_err(RenderError::serialize)
            }
            OutputFormat::Yaml => serde_yaml::to_string(value).map_err(RenderError::serialize),
            OutputFormat::JsonPath(expr) => {
                let as_value = serde_json::to_value(value).map_err(RenderError::serialize)?;
                jsonpath_extract(&as_value, expr)
                    .map_err(RenderError::JsonPath)
                    .map(|v| match v {
                        // Strings render without quotes so `-o jsonpath={.name}`
                        // is shell-pipelineable without post-processing.
                        Value::String(s) => s,
                        other => other.to_string(),
                    })
            }
        }
    }

    /// Render with an explicit `name_field` for the `Name` format.
    ///
    /// When `format == Name` and `value` serialises to an object or
    /// array-of-objects containing `name_field`, emits one value per
    /// line. Otherwise delegates to [`Self::render_structured`].
    ///
    /// # Errors
    /// Same as [`Self::render_structured`], plus
    /// [`RenderError::JsonPath`] if the named field is missing.
    pub fn render_with_name<T: Serialize>(
        &self,
        value: &T,
        name_field: &str,
    ) -> Result<String, RenderError> {
        if let OutputFormat::Name = self.format {
            let v = serde_json::to_value(value).map_err(RenderError::serialize)?;
            return render_names(&v, name_field).map_err(RenderError::JsonPath);
        }
        self.render_structured(value)
    }
}

/// Error returned from [`Renderer`] rendering operations.
#[derive(Debug)]
pub enum RenderError {
    /// Failure in `serde_json` / `serde_yaml` encoding.
    Serialize(Box<dyn std::error::Error + Send + Sync>),
    /// Failure during JSONPath extraction.
    JsonPath(JsonPathError),
}

impl RenderError {
    fn serialize<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Serialize(Box::new(e))
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "failed to serialise output: {e}"),
            Self::JsonPath(e) => write!(f, "jsonpath extraction failed: {e}"),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(e) => Some(&**e),
            Self::JsonPath(e) => Some(e),
        }
    }
}

fn render_names(value: &Value, name_field: &str) -> Result<String, JsonPathError> {
    match value {
        Value::Array(items) => {
            let mut out = String::new();
            for item in items {
                let name = item
                    .get(name_field)
                    .ok_or_else(|| JsonPathError::MissingField {
                        field: name_field.to_string(),
                    })?;
                out.push_str(stringify_value(name).as_str());
                out.push('\n');
            }
            // No trailing newline — callers add one with `println!`
            // if they want it.
            if out.ends_with('\n') {
                out.pop();
            }
            Ok(out)
        }
        Value::Object(_) => {
            let name = value
                .get(name_field)
                .ok_or_else(|| JsonPathError::MissingField {
                    field: name_field.to_string(),
                })?;
            Ok(stringify_value(name))
        }
        _ => Err(JsonPathError::NotAnObject {
            at: "<root>".into(),
        }),
    }
}

fn stringify_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Minimal kubectl-compatible JSONPath extractor
// ---------------------------------------------------------------------------

/// Apply a kubectl-style JSONPath expression to a JSON value.
///
/// Supported syntax:
/// - optional `{` `}` wrapper (`{.foo}` and `.foo` are equivalent)
/// - leading `.` is optional (`foo.bar` and `.foo.bar` are equivalent)
/// - `.field` for object access (field must not contain `.` or `[`)
/// - `[N]` for zero-based array indexing
///
/// Unsupported (returns [`JsonPathError::UnsupportedConstruct`]):
/// - `[*]` wildcards
/// - `[?(...)]` filters
/// - `[start:end]` slices
/// - `..` recursive descent
///
/// # Errors
/// Returns a [`JsonPathError`] variant describing the failure.
pub fn jsonpath_extract(value: &Value, expr: &str) -> Result<Value, JsonPathError> {
    let expr = strip_braces(expr.trim());
    // Recursive descent (`..`) must be caught before tokenisation
    // strips the leading dot; `..foo` and `.foo..bar` are both
    // invalid but structurally different to spot mid-token.
    if expr.contains("..") {
        return Err(JsonPathError::UnsupportedConstruct(
            "recursive descent (`..`)".into(),
        ));
    }
    let tokens = tokenize(expr)?;

    let mut current = value;
    let mut trail = String::new();
    for tok in tokens {
        match tok {
            Token::Field(name) => {
                let Value::Object(map) = current else {
                    return Err(JsonPathError::NotAnObject { at: trail });
                };
                current = map.get(&name).ok_or_else(|| JsonPathError::MissingField {
                    field: name.clone(),
                })?;
                trail.push('.');
                trail.push_str(&name);
            }
            Token::Index(idx) => {
                let Value::Array(arr) = current else {
                    return Err(JsonPathError::NotAnArray { at: trail });
                };
                let len = arr.len();
                current = arr
                    .get(idx)
                    .ok_or(JsonPathError::IndexOutOfBounds { index: idx, len })?;
                trail.push_str(&format!("[{idx}]"));
            }
        }
    }

    Ok(current.clone())
}

fn strip_braces(expr: &str) -> &str {
    expr.strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .unwrap_or(expr)
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Field(String),
    Index(usize),
}

fn tokenize(expr: &str) -> Result<Vec<Token>, JsonPathError> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    // Leading '.' is optional but conventional (kubectl emits it).
    if let Some(&'.') = chars.peek() {
        chars.next();
    }

    while let Some(&c) = chars.peek() {
        match c {
            '.' => {
                chars.next();
                // `..` recursive descent is unsupported.
                if let Some(&'.') = chars.peek() {
                    return Err(JsonPathError::UnsupportedConstruct(
                        "recursive descent (`..`)".into(),
                    ));
                }
            }
            '[' => {
                chars.next();
                let mut buf = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == ']' {
                        closed = true;
                        break;
                    }
                    buf.push(ch);
                }
                if !closed {
                    return Err(JsonPathError::MalformedExpression(
                        "unterminated `[`".into(),
                    ));
                }
                let inner = buf.trim();
                if inner == "*" {
                    return Err(JsonPathError::UnsupportedConstruct(
                        "wildcard (`[*]`)".into(),
                    ));
                }
                if inner.contains(':') {
                    return Err(JsonPathError::UnsupportedConstruct(
                        "slice (`[start:end]`)".into(),
                    ));
                }
                if inner.starts_with('?') {
                    return Err(JsonPathError::UnsupportedConstruct(
                        "filter (`[?(...)]`)".into(),
                    ));
                }
                let idx = inner.parse::<usize>().map_err(|_| {
                    JsonPathError::MalformedExpression(format!("not a numeric index: `{inner}`"))
                })?;
                tokens.push(Token::Index(idx));
            }
            c if c.is_alphanumeric() || c == '_' || c == '-' => {
                let mut buf = String::new();
                while let Some(&cc) = chars.peek() {
                    if cc == '.' || cc == '[' {
                        break;
                    }
                    buf.push(cc);
                    chars.next();
                }
                tokens.push(Token::Field(buf));
            }
            other => {
                return Err(JsonPathError::MalformedExpression(format!(
                    "unexpected character `{other}`"
                )));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- format parsing --------------------------------------------------

    #[test]
    fn parse_accepts_all_four_bare_keywords_case_insensitively() {
        for (input, expected) in [
            ("table", OutputFormat::Table),
            ("Table", OutputFormat::Table),
            ("TABLE", OutputFormat::Table),
            ("json", OutputFormat::Json),
            ("JSON", OutputFormat::Json),
            ("yaml", OutputFormat::Yaml),
            ("name", OutputFormat::Name),
        ] {
            assert_eq!(OutputFormat::parse(input).unwrap(), expected);
        }
    }

    #[test]
    fn parse_strips_surrounding_whitespace() {
        assert_eq!(OutputFormat::parse("  json  ").unwrap(), OutputFormat::Json);
    }

    #[test]
    fn parse_rejects_unknown_keyword_with_full_help() {
        let err = OutputFormat::parse("toml").unwrap_err();
        assert!(matches!(err, ParseFormatError::Unknown(ref s) if s == "toml"));
        assert!(
            err.to_string()
                .contains("table, json, yaml, name, jsonpath")
        );
    }

    #[test]
    fn parse_jsonpath_captures_expression_verbatim() {
        assert_eq!(
            OutputFormat::parse("jsonpath={.items[0].name}").unwrap(),
            OutputFormat::JsonPath("{.items[0].name}".into())
        );
        // `=` inside the expression stays with the expression — only
        // the first `=` is a separator.
        assert_eq!(
            OutputFormat::parse("jsonpath=.foo=bar").unwrap(),
            OutputFormat::JsonPath(".foo=bar".into())
        );
    }

    #[test]
    fn parse_rejects_empty_jsonpath() {
        assert_eq!(
            OutputFormat::parse("jsonpath=").unwrap_err(),
            ParseFormatError::EmptyJsonPath
        );
    }

    // ---- default_for_tty (task 9.2) -------------------------------------

    #[test]
    fn default_is_table_on_tty_and_json_in_pipe() {
        assert_eq!(OutputFormat::default_for_tty(true), OutputFormat::Table);
        assert_eq!(OutputFormat::default_for_tty(false), OutputFormat::Json);
    }

    // ---- Display ---------------------------------------------------------

    #[test]
    fn display_roundtrips_through_parse_for_bare_keywords() {
        for f in [
            OutputFormat::Table,
            OutputFormat::Json,
            OutputFormat::Yaml,
            OutputFormat::Name,
        ] {
            let s = f.to_string();
            assert_eq!(OutputFormat::parse(&s).unwrap(), f);
        }
    }

    #[test]
    fn display_of_jsonpath_roundtrips() {
        let f = OutputFormat::JsonPath("{.foo}".into());
        assert_eq!(f.to_string(), "jsonpath={.foo}");
        assert_eq!(OutputFormat::parse(&f.to_string()).unwrap(), f);
    }

    // ---- Renderer: Json / Yaml ------------------------------------------

    #[test]
    fn render_json_is_pretty() {
        let r = Renderer::new(OutputFormat::Json);
        let out = r.render_structured(&json!({"a": 1})).unwrap();
        assert!(out.contains('\n'), "pretty JSON must have newlines");
        assert!(out.contains("\"a\""));
    }

    #[test]
    fn render_yaml_emits_valid_yaml() {
        let r = Renderer::new(OutputFormat::Yaml);
        let out = r.render_structured(&json!({"a": 1, "b": "two"})).unwrap();
        let round: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(round["a"], serde_yaml::Value::Number(1.into()));
        assert_eq!(round["b"], serde_yaml::Value::String("two".into()));
    }

    #[test]
    fn render_table_falls_back_to_pretty_json_for_generic_values() {
        // Generic structured values have no column layout, so Table
        // mode falls back to the pretty JSON shape; commands that
        // want a real table branch on format() first.
        let r = Renderer::new(OutputFormat::Table);
        let out = r.render_structured(&json!({"a": 1})).unwrap();
        assert!(out.contains("\"a\""));
    }

    // ---- Renderer: JsonPath ---------------------------------------------

    #[test]
    fn render_jsonpath_extracts_string_without_quotes() {
        let r = Renderer::new(OutputFormat::JsonPath("{.name}".into()));
        let out = r
            .render_structured(&json!({"name": "acme", "other": 1}))
            .unwrap();
        // Not `"acme"` — bare, shell-pipelineable.
        assert_eq!(out, "acme");
    }

    #[test]
    fn render_jsonpath_serialises_non_strings_as_json_literals() {
        let r = Renderer::new(OutputFormat::JsonPath(".count".into()));
        let out = r.render_structured(&json!({"count": 42})).unwrap();
        assert_eq!(out, "42");
    }

    #[test]
    fn render_jsonpath_navigates_nested_arrays() {
        let r = Renderer::new(OutputFormat::JsonPath("{.items[1].name}".into()));
        let out = r
            .render_structured(&json!({
                "items": [
                    {"name": "a"},
                    {"name": "b"},
                    {"name": "c"},
                ]
            }))
            .unwrap();
        assert_eq!(out, "b");
    }

    #[test]
    fn render_jsonpath_surfaces_missing_field_as_rendererror() {
        let r = Renderer::new(OutputFormat::JsonPath(".nope".into()));
        let err = r.render_structured(&json!({"a": 1})).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("missing field 'nope'"), "got: {msg}");
    }

    // ---- Renderer: Name --------------------------------------------------

    #[test]
    fn render_name_emits_one_entry_per_line_from_array() {
        let r = Renderer::new(OutputFormat::Name);
        let out = r
            .render_with_name(
                &json!([
                    {"slug": "alpha"},
                    {"slug": "beta"},
                    {"slug": "gamma"},
                ]),
                "slug",
            )
            .unwrap();
        // No trailing newline — callers add one with println! if they
        // want it.
        assert_eq!(out, "alpha\nbeta\ngamma");
    }

    #[test]
    fn render_name_on_single_object_emits_one_value() {
        let r = Renderer::new(OutputFormat::Name);
        let out = r
            .render_with_name(&json!({"slug": "solo"}), "slug")
            .unwrap();
        assert_eq!(out, "solo");
    }

    #[test]
    fn render_name_missing_field_fails_with_clear_error() {
        let r = Renderer::new(OutputFormat::Name);
        let err = r
            .render_with_name(&json!({"other": "x"}), "slug")
            .unwrap_err();
        assert!(err.to_string().contains("missing field 'slug'"));
    }

    #[test]
    fn render_name_delegates_to_structured_when_format_is_not_name() {
        // JSON mode ignores the name_field — the value is rendered
        // verbatim rather than filtered.
        let r = Renderer::new(OutputFormat::Json);
        let out = r
            .render_with_name(&json!({"slug": "x", "other": 1}), "slug")
            .unwrap();
        assert!(out.contains("\"slug\""));
        assert!(out.contains("\"other\""));
    }

    // ---- jsonpath_extract direct -----------------------------------------

    #[test]
    fn jsonpath_braces_are_optional() {
        let v = json!({"a": {"b": 1}});
        assert_eq!(jsonpath_extract(&v, "{.a.b}").unwrap(), json!(1));
        assert_eq!(jsonpath_extract(&v, ".a.b").unwrap(), json!(1));
        assert_eq!(jsonpath_extract(&v, "a.b").unwrap(), json!(1));
    }

    #[test]
    fn jsonpath_field_on_non_object_reports_path() {
        let v = json!({"a": 1});
        // Walked `.a` successfully, then tried `.b` on an integer.
        assert_eq!(
            jsonpath_extract(&v, ".a.b").unwrap_err(),
            JsonPathError::NotAnObject { at: ".a".into() }
        );
    }

    #[test]
    fn jsonpath_index_on_non_array_reports_path() {
        let v = json!({"a": 1});
        assert_eq!(
            jsonpath_extract(&v, ".a[0]").unwrap_err(),
            JsonPathError::NotAnArray { at: ".a".into() }
        );
    }

    #[test]
    fn jsonpath_index_out_of_bounds_includes_len() {
        let v = json!({"a": [1, 2]});
        assert_eq!(
            jsonpath_extract(&v, ".a[5]").unwrap_err(),
            JsonPathError::IndexOutOfBounds { index: 5, len: 2 }
        );
    }

    #[test]
    fn jsonpath_wildcard_is_unsupported_with_clear_message() {
        let v = json!({"items": [1, 2]});
        let err = jsonpath_extract(&v, ".items[*]").unwrap_err();
        assert!(matches!(err, JsonPathError::UnsupportedConstruct(_)));
        assert!(err.to_string().contains("wildcard"));
    }

    #[test]
    fn jsonpath_slice_is_unsupported() {
        let v = json!({"items": [1, 2, 3]});
        assert!(matches!(
            jsonpath_extract(&v, ".items[0:2]").unwrap_err(),
            JsonPathError::UnsupportedConstruct(_)
        ));
    }

    #[test]
    fn jsonpath_filter_is_unsupported() {
        let v = json!({"items": [{"ok": true}]});
        assert!(matches!(
            jsonpath_extract(&v, ".items[?(@.ok==true)]").unwrap_err(),
            JsonPathError::UnsupportedConstruct(_)
        ));
    }

    #[test]
    fn jsonpath_recursive_descent_is_unsupported() {
        let v = json!({"a": {"b": 1}});
        assert!(matches!(
            jsonpath_extract(&v, "..b").unwrap_err(),
            JsonPathError::UnsupportedConstruct(_)
        ));
    }

    #[test]
    fn jsonpath_unterminated_bracket_is_malformed() {
        let v = json!({"a": [1]});
        assert!(matches!(
            jsonpath_extract(&v, ".a[0").unwrap_err(),
            JsonPathError::MalformedExpression(_)
        ));
    }

    #[test]
    fn jsonpath_non_numeric_index_is_malformed() {
        let v = json!({"a": [1]});
        assert!(matches!(
            jsonpath_extract(&v, ".a[foo]").unwrap_err(),
            JsonPathError::MalformedExpression(_)
        ));
    }

    #[test]
    fn jsonpath_field_with_underscore_and_hyphen_is_supported() {
        let v = json!({"my_field-name": 7});
        assert_eq!(jsonpath_extract(&v, ".my_field-name").unwrap(), json!(7));
    }
}
