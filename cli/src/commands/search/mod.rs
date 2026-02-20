//! # Code Search CLI Commands
//!
//! CLI commands for code search and call graph analysis.

pub mod index;
pub mod init;
pub mod repo;
pub mod search;
pub mod status;
pub mod trace;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum CodeSearchCommand {
    #[command(about = "Initialize code search for a project directory")]
    Init(init::InitArgs),

    #[command(about = "Search code using semantic queries")]
    Search(search::SearchArgs),

    #[command(about = "Trace function callers or callees")]
    Trace(trace::TraceArgs),

    #[command(about = "Show indexing status")]
    Status(status::StatusArgs),

    #[command(about = "Manage repositories")]
    Repo(repo::RepoArgs),

    #[command(about = "Trigger repository re-indexing")]
    Index(index::IndexArgs),
}

pub async fn handle_command(cmd: CodeSearchCommand) -> anyhow::Result<()> {
    match cmd {
        CodeSearchCommand::Init(args) => init::handle(args).await,
        CodeSearchCommand::Search(args) => search::handle(args).await,
        CodeSearchCommand::Trace(args) => trace::handle(args).await,
        CodeSearchCommand::Status(args) => status::handle(args).await,
        CodeSearchCommand::Repo(args) => repo::handle(args).await,
        CodeSearchCommand::Index(args) => index::handle(args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_init_args_defaults() {
        let args = init::InitArgs {
            path: PathBuf::from("."),
            embedder: "ollama".to_string(),
            model: None,
            store: "gob".to_string(),
            qdrant_url: None,
            postgres_url: None,
            force: false,
            json: false,
        };
        assert_eq!(args.embedder, "ollama");
        assert_eq!(args.store, "gob");
        assert!(!args.force);
        assert!(!args.json);
        assert!(args.model.is_none());
    }

    #[test]
    fn test_init_args_with_qdrant() {
        let args = init::InitArgs {
            path: PathBuf::from("/tmp/myproject"),
            embedder: "openai".to_string(),
            model: Some("text-embedding-3-small".to_string()),
            store: "qdrant".to_string(),
            qdrant_url: Some("http://localhost:6333".to_string()),
            postgres_url: None,
            force: true,
            json: false,
        };
        assert_eq!(args.store, "qdrant");
        assert_eq!(args.qdrant_url.as_deref(), Some("http://localhost:6333"));
        assert!(args.force);
    }

    #[test]
    fn test_search_args_defaults() {
        let args = search::SearchArgs {
            query: "authentication middleware".to_string(),
            limit: 10,
            threshold: 0.7,
            file_pattern: None,
            language: None,
            json: false,
            files_only: false,
        };
        assert_eq!(args.query, "authentication middleware");
        assert_eq!(args.limit, 10);
        assert!((args.threshold - 0.7).abs() < f32::EPSILON);
        assert!(args.file_pattern.is_none());
        assert!(!args.json);
        assert!(!args.files_only);
    }

    #[test]
    fn test_search_args_with_filters() {
        let args = search::SearchArgs {
            query: "error handling".to_string(),
            limit: 5,
            threshold: 0.85,
            file_pattern: Some("src/**/*.rs".to_string()),
            language: Some("rust".to_string()),
            json: true,
            files_only: false,
        };
        assert_eq!(args.limit, 5);
        assert_eq!(args.language.as_deref(), Some("rust"));
        assert_eq!(args.file_pattern.as_deref(), Some("src/**/*.rs"));
        assert!(args.json);
    }

    #[test]
    fn test_status_args_defaults() {
        let args = status::StatusArgs {
            project: None,
            json: false,
            watch: false,
        };
        assert!(args.project.is_none());
        assert!(!args.watch);
    }

    #[test]
    fn test_status_args_with_project() {
        let args = status::StatusArgs {
            project: Some("payments-service".to_string()),
            json: false,
            watch: true,
        };
        assert_eq!(args.project.as_deref(), Some("payments-service"));
        assert!(args.watch);
    }

    #[test]
    fn test_callers_args_defaults() {
        let args = trace::CallersArgs {
            symbol: "handle_payment".to_string(),
            file: None,
            recursive: false,
            max_depth: 3,
            json: false,
        };
        assert_eq!(args.symbol, "handle_payment");
        assert_eq!(args.max_depth, 3);
        assert!(!args.recursive);
        assert!(args.file.is_none());
    }

    #[test]
    fn test_callees_args_with_file() {
        let args = trace::CalleesArgs {
            symbol: "process_request".to_string(),
            file: Some("src/handler.rs".to_string()),
            recursive: true,
            max_depth: 5,
            json: true,
        };
        assert_eq!(args.file.as_deref(), Some("src/handler.rs"));
        assert!(args.recursive);
        assert_eq!(args.max_depth, 5);
    }

    #[test]
    fn test_graph_args_defaults() {
        let args = trace::GraphArgs {
            symbol: "MyStruct::new".to_string(),
            file: None,
            depth: 2,
            include_callers: true,
            include_callees: true,
            format: "json".to_string(),
        };
        assert_eq!(args.depth, 2);
        assert!(args.include_callers);
        assert!(args.include_callees);
        assert_eq!(args.format, "json");
    }

    #[test]
    fn test_codesearch_command_variants_constructible() {
        let _ = CodeSearchCommand::Init(init::InitArgs {
            path: PathBuf::from("."),
            embedder: "ollama".to_string(),
            model: None,
            store: "gob".to_string(),
            qdrant_url: None,
            postgres_url: None,
            force: false,
            json: false,
        });
        let _ = CodeSearchCommand::Status(status::StatusArgs {
            project: None,
            json: false,
            watch: false,
        });
    }
}
