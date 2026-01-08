use serde::{Deserialize, Serialize};

/// Knowledge types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeType {
    /// Architecture Decision Records
    Adr,

    /// Policy documents
    Policy,

    /// Design patterns
    Pattern,

    /// Specifications
    Spec,
}

/// Knowledge layers for hierarchical organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeLayer {
    /// Company-wide knowledge
    Company,

    /// Organization-level knowledge
    Org,

    /// Team-specific knowledge
    Team,

    /// Project-specific knowledge
    Project,
}

/// Constraint severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintSeverity {
    /// Informational only
    Info,

    /// Warning level
    Warn,

    /// Blocking violation
    Block,
}

/// Constraint operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintOperator {
    /// Must use this item
    MustUse,

    /// Must not use this item
    MustNotUse,

    /// Must match pattern
    MustMatch,

    /// Must not match pattern
    MustNotMatch,

    /// Must exist
    MustExist,

    /// Must not exist
    MustNotExist,
}

/// Constraint targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintTarget {
    /// File-based constraint
    File,

    /// Code-based constraint
    Code,

    /// Dependency-based constraint
    Dependency,

    /// Import-based constraint
    Import,

    /// Config-based constraint
    Config,
}

/// Memory layers for hierarchical storage
///
/// 7-layer hierarchy with precedence rules:
/// - Priority 1 (highest): agent
/// - Priority 2: user
/// - Priority 3: session
/// - Priority 4: project
/// - Priority 5: team
/// - Priority 6: org
/// - Priority 7 (lowest): company
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryLayer {
    /// Per-agent instance (most specific)
    Agent,

    /// Cross-session user data
    User,

    /// Single conversation context
    Session,

    /// Project-wide persistent data
    Project,

    /// Team-shared knowledge
    Team,

    /// Organization-level policies
    Org,

    /// Company-wide standards
    Company,
}

impl MemoryLayer {
    /// Returns precedence value (1=highest, 7=lowest)
    #[must_use]
    pub fn precedence(&self) -> u8 {
        match self {
            MemoryLayer::Agent => 1,
            MemoryLayer::User => 2,
            MemoryLayer::Session => 3,
            MemoryLayer::Project => 4,
            MemoryLayer::Team => 5,
            MemoryLayer::Org => 6,
            MemoryLayer::Company => 7,
        }
    }

    /// Returns layer display name
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryLayer::Agent => "Agent",
            MemoryLayer::User => "User",
            MemoryLayer::Session => "Session",
            MemoryLayer::Project => "Project",
            MemoryLayer::Team => "Team",
            MemoryLayer::Org => "Organization",
            MemoryLayer::Company => "Company",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub layer: MemoryLayer,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
}
