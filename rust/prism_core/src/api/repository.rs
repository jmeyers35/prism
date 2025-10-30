use serde::{Deserialize, Serialize};

/// Basic information about the repository Prism is operating on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryInfo {
    /// Absolute path to the repository working tree.
    pub root: String,
    /// Default branch name when available (e.g., "main").
    #[serde(default)]
    pub default_branch: Option<String>,
}

/// Identity of a revision that Prism can reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// Full object identifier (e.g., git SHA).
    pub oid: String,
    /// Optional human-friendly reference such as a branch name.
    #[serde(default)]
    pub reference: Option<String>,
    /// Optional summary line describing the revision.
    #[serde(default)]
    pub summary: Option<String>,
    /// Author information when available.
    #[serde(default)]
    pub author: Option<Signature>,
    /// Committer information when available.
    #[serde(default)]
    pub committer: Option<Signature>,
    /// Unix timestamp (seconds) associated with the revision.
    #[serde(default)]
    pub timestamp: Option<i64>,
}

/// Structured author/committer identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Display name for the individual.
    pub name: String,
    /// Optional email address.
    #[serde(default)]
    pub email: Option<String>,
}

/// The pair of revisions used for diff operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionRange {
    /// Base revision (often the "previous" commit). May be omitted for new files.
    #[serde(default)]
    pub base: Option<Revision>,
    /// Head revision (the state being reviewed).
    pub head: Revision,
}

/// Lightweight summary of the workspace status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    /// Name of the currently checked-out branch, if any.
    #[serde(default)]
    pub current_branch: Option<String>,
    /// Indicates if there are uncommitted modifications.
    pub dirty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_round_trip() {
        let revision = Revision {
            oid: "0123456789abcdef0123456789abcdef01234567".into(),
            reference: Some("feature/example".into()),
            summary: Some("Add example module".into()),
            author: Some(Signature {
                name: "Example Author".into(),
                email: Some("author@example.com".into()),
            }),
            committer: None,
            timestamp: Some(1_690_000_000),
        };

        let json = serde_json::to_string(&revision).expect("serialize revision");
        let decoded: Revision = serde_json::from_str(&json).expect("deserialize revision");
        assert_eq!(revision, decoded);
    }

    #[test]
    fn repository_info_defaults() {
        let json = r#"{
            "root": "/tmp/prism",
            "default_branch": null
        }"#;
        let info: RepositoryInfo = serde_json::from_str(json).expect("deserialize info");
        assert_eq!(info.root, "/tmp/prism");
        assert!(info.default_branch.is_none());
    }

    #[test]
    fn workspace_status_serializes() {
        let status = WorkspaceStatus {
            current_branch: Some("feature".into()),
            dirty: true,
        };
        let json = serde_json::to_string(&status).expect("serialize status");
        assert!(json.contains("\"dirty\":true"));
    }

    #[test]
    fn revision_range_head_required() {
        let json = r#"{
            "base": null,
            "head": {
                "oid": "fedcba9876543210fedcba9876543210fedcba98"
            }
        }"#;

        let range: RevisionRange = serde_json::from_str(json).expect("deserialize range");
        assert_eq!(range.head.oid, "fedcba9876543210fedcba9876543210fedcba98");
        assert!(range.base.is_none());
    }
}
