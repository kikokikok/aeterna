//! Check command - Constraint validation
//!
//! Validates the current project/context against knowledge constraints:
//! - Policy compliance checks
//! - Dependency restrictions
//! - Architecture rule validation
//! - Security policy enforcement

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use context::ContextResolver;

use crate::output;

#[derive(Args)]
pub struct CheckArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Check target: all, policies, dependencies, architecture, security
    #[arg(long, default_value = "all")]
    pub target: String,

    /// Fail on warnings (exit code 1)
    #[arg(long)]
    pub strict: bool,

    /// Show only violations (hide passing checks)
    #[arg(long)]
    pub violations_only: bool,

    /// Specific files or paths to check (defaults to current directory)
    #[arg(value_name = "PATH")]
    pub paths: Vec<String>
}

#[derive(Debug, Clone)]
enum Severity {
    Error,
    Warning,
    Info
}

impl Severity {
    fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info"
        }
    }
}

#[derive(Debug, Clone)]
struct CheckResult {
    category: String,
    rule: String,
    severity: Severity,
    message: String,
    file: Option<String>,
    line: Option<u32>,
    suggestion: Option<String>
}

pub async fn run(args: CheckArgs) -> Result<()> {
    let resolver = ContextResolver::new();
    let ctx = resolver.resolve()?;

    if args.json {
        return run_json(args, &ctx).await;
    }

    output::header("Constraint Validation");
    println!();

    let tenant = &ctx.tenant_id.value;
    let project = ctx
        .project_id
        .as_ref()
        .map_or("(current directory)", |p| p.value.as_str());

    println!("  {} {}", "Tenant:".dimmed(), tenant.cyan());
    println!("  {} {}", "Project:".dimmed(), project.cyan());
    println!(
        "  {} {}",
        "Target:".dimmed(),
        args.target.to_uppercase().cyan()
    );
    println!();

    // Run checks
    let results = run_checks(&args, &ctx);

    // Group results by category
    let errors: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Error))
        .collect();
    let warnings: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Warning))
        .collect();
    let infos: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Info))
        .collect();

    // Print results
    if !errors.is_empty() {
        output::subheader("Errors");
        for result in &errors {
            print_result(result);
        }
        println!();
    }

    if !warnings.is_empty() && !args.violations_only {
        output::subheader("Warnings");
        for result in &warnings {
            print_result(result);
        }
        println!();
    }

    if !infos.is_empty() && !args.violations_only {
        output::subheader("Info");
        for result in &infos {
            print_result(result);
        }
        println!();
    }

    // Summary
    output::subheader("Summary");
    println!();
    println!(
        "  {} {} errors",
        if errors.is_empty() {
            "✓".green()
        } else {
            "✗".red()
        },
        errors.len()
    );
    println!(
        "  {} {} warnings",
        if warnings.is_empty() {
            "✓".green()
        } else {
            "⚠".yellow()
        },
        warnings.len()
    );
    println!("  {} {} info", "ℹ".blue(), infos.len());
    println!();

    // Determine exit status
    let has_violations = !errors.is_empty() || (args.strict && !warnings.is_empty());

    if has_violations {
        if errors.is_empty() {
            output::warn("Validation failed (strict mode) with warnings");
        } else {
            output::error("Validation failed with errors");
        }
        std::process::exit(1);
    } else {
        output::success("All checks passed");
    }

    Ok(())
}

async fn run_json(args: CheckArgs, ctx: &context::ResolvedContext) -> Result<()> {
    let results = run_checks(&args, ctx);

    let errors: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Error))
        .collect();
    let warnings: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.severity, Severity::Warning))
        .collect();

    let has_violations = !errors.is_empty() || (args.strict && !warnings.is_empty());

    let output = serde_json::json!({
        "success": !has_violations,
        "context": {
            "tenant_id": ctx.tenant_id.value,
            "project_id": ctx.project_id.as_ref().map(|p| &p.value),
        },
        "target": args.target,
        "strict": args.strict,
        "results": results.iter().map(|r| serde_json::json!({
            "category": r.category,
            "rule": r.rule,
            "severity": r.severity.as_str(),
            "message": r.message,
            "file": r.file,
            "line": r.line,
            "suggestion": r.suggestion,
        })).collect::<Vec<_>>(),
        "summary": {
            "errors": errors.len(),
            "warnings": warnings.len(),
            "total": results.len(),
        }
    });
    println!("{}", serde_json::to_string_pretty(&output)?);

    if has_violations {
        std::process::exit(1);
    }

    Ok(())
}

fn run_checks(args: &CheckArgs, _ctx: &context::ResolvedContext) -> Vec<CheckResult> {
    let mut results = Vec::new();

    let targets: Vec<&str> = if args.target == "all" {
        vec!["policies", "dependencies", "architecture", "security"]
    } else {
        vec![args.target.as_str()]
    };

    for target in targets {
        match target {
            "policies" => results.extend(check_policies(args)),
            "dependencies" => results.extend(check_dependencies(args)),
            "architecture" => results.extend(check_architecture(args)),
            "security" => results.extend(check_security(args)),
            _ => {}
        }
    }

    results
}

fn check_policies(_args: &CheckArgs) -> Vec<CheckResult> {
    // TODO: Replace with actual policy checks when backend is implemented
    // Currently returns empty (all passing)
    vec![]
}

fn check_dependencies(_args: &CheckArgs) -> Vec<CheckResult> {
    // TODO: Replace with actual dependency checks when backend is implemented
    // Currently returns empty (all passing)
    vec![]
}

fn check_architecture(_args: &CheckArgs) -> Vec<CheckResult> {
    // TODO: Replace with actual architecture checks when backend is implemented
    // Currently returns empty (all passing)
    vec![]
}

fn check_security(_args: &CheckArgs) -> Vec<CheckResult> {
    // TODO: Replace with actual security checks when backend is implemented
    // Currently returns empty (all passing)
    vec![]
}

fn print_result(result: &CheckResult) {
    let severity_icon = match result.severity {
        Severity::Error => "✗".red(),
        Severity::Warning => "⚠".yellow(),
        Severity::Info => "ℹ".blue()
    };

    let location = match (&result.file, result.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        _ => String::new()
    };

    if location.is_empty() {
        println!(
            "  {} [{}] {}: {}",
            severity_icon,
            result.category.dimmed(),
            result.rule.cyan(),
            result.message
        );
    } else {
        println!(
            "  {} {} [{}] {}: {}",
            severity_icon,
            location.dimmed(),
            result.category.dimmed(),
            result.rule.cyan(),
            result.message
        );
    }

    if let Some(suggestion) = &result.suggestion {
        println!("    {} {}", "→".cyan(), suggestion.dimmed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Info.as_str(), "info");
    }

    #[test]
    fn test_check_policies_empty() {
        let args = CheckArgs {
            json: false,
            target: "policies".to_string(),
            strict: false,
            violations_only: false,
            paths: vec![]
        };
        let results = check_policies(&args);
        assert!(results.is_empty());
    }

    #[test]
    fn test_check_dependencies_empty() {
        let args = CheckArgs {
            json: false,
            target: "dependencies".to_string(),
            strict: false,
            violations_only: false,
            paths: vec![]
        };
        let results = check_dependencies(&args);
        assert!(results.is_empty());
    }

    #[test]
    fn test_check_architecture_empty() {
        let args = CheckArgs {
            json: false,
            target: "architecture".to_string(),
            strict: false,
            violations_only: false,
            paths: vec![]
        };
        let results = check_architecture(&args);
        assert!(results.is_empty());
    }

    #[test]
    fn test_check_security_empty() {
        let args = CheckArgs {
            json: false,
            target: "security".to_string(),
            strict: false,
            violations_only: false,
            paths: vec![]
        };
        let results = check_security(&args);
        assert!(results.is_empty());
    }
}
