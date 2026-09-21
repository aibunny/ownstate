//! Capture mode, scope resolution, and scope-handle domain types.
//!
//! These types are pure domain contracts with no infrastructure dependencies.
//! They model the automatic capture boundary and scope resolution output.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::enums::{CaptureMode, ChannelCompleteness, ResolutionState, ScopeKind};
use crate::ids::*;

/// A server-minted opaque scope handle. It is a convenience reference, not
/// canonical identity, authentication, authorization, or a bearer capability.
/// Bound to an ownership domain and resolved records; on every use,
/// authenticate independently and re-authorize the referenced resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeHandle {
    pub id: ScopeHandleId,
    pub tenant_id: TenantId,
    pub ownership_domain: crate::enums::OwnershipDomain,
    pub project_id: Option<ProjectId>,
    pub repository_id: Option<RepositoryId>,
    pub workspace_id: Option<WorkspaceId>,
    pub scope_kind: ScopeKind,
    pub resolution_state: ResolutionState,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl ScopeHandle {
    /// A handle is usable when it exists, has not been revoked, and has not
    /// expired.
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none()
            && (self.expires_at.is_none() || self.expires_at.is_some_and(|exp| exp > now))
    }
}

/// Normalized repository identity locator. Credential-stripped, deterministic,
/// provider-neutral representation of a remote repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryLocator {
    /// Canonical URI after normalization (credential-stripped, normalized).
    pub canonical_uri: String,
    /// Provider hint (github, gitlab, bitbucket, self_hosted, unknown).
    pub provider: RepositoryProvider,
    /// Owner/org name (preserving provider-specific case).
    pub owner: Option<String>,
    /// Repository name (preserving provider-specific case).
    pub repository_name: Option<String>,
    /// Stable internal key derived from canonical identity. Never a user-managed
    /// identifier.
    pub internal_key: String,
    /// Whether this is a fork and the upstream locator.
    pub upstream: Option<Box<RepositoryLocator>>,
}

/// Repository hosting provider hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RepositoryProvider {
    GitHub,
    GitLab,
    Bitbucket,
    SelfHosted,
    Unknown,
}

impl RepositoryProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "GITHUB",
            Self::GitLab => "GITLAB",
            Self::Bitbucket => "BITBUCKET",
            Self::SelfHosted => "SELF_HOSTED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for RepositoryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Capture assessment for a session or source. Records factual capture quality
/// metadata. Append-oriented and auditable; later assessments add new records
/// without rewriting history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureAssessment {
    pub id: CaptureAssessmentId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub repository_id: Option<RepositoryId>,
    pub capture_mode: CaptureMode,
    pub channels: ChannelCompleteness,
    pub source_adapter: String,
    pub assessed_at: DateTime<Utc>,
    pub assessed_by: Option<String>,
    pub notes: Option<String>,
}

/// Scope resolution result returned by the ScopeResolver. Identifies which
/// anchors and deterministic rules produced the result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedScope {
    pub scope_kind: ScopeKind,
    pub resolution_state: ResolutionState,
    pub project_id: Option<ProjectId>,
    pub repository_id: Option<RepositoryId>,
    pub workspace_id: Option<WorkspaceId>,
    pub ownership_domain: crate::enums::OwnershipDomain,
    pub classification_ceiling: crate::enums::SecurityClassification,
    /// Which anchors produced this resolution.
    pub resolution_anchors: Vec<ResolutionAnchor>,
    /// Opaque handle for subsequent MCP operations.
    pub handle: ScopeHandle,
}

/// An anchor that contributed to scope resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionAnchor {
    pub anchor_type: ResolutionAnchorType,
    pub evidence: String,
    pub confidence: f32,
}

/// Types of deterministic anchors used in scope resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResolutionAnchorType {
    CanonicalRemote,
    KnownAlias,
    RepositoryMetadata,
    ProvisionalLocal,
    WorkspacePath,
    ExplicitHandle,
    EntityAssociation,
    ThreadLineage,
    RecentAssociation,
    SemanticSimilarity,
}

/// Bootstrap request from an MCP client. All fields are untrusted and bounded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapRequest {
    /// Workspace/directory name (untrusted).
    pub workspace_name: Option<String>,
    /// Git remote URL (untrusted, may contain credentials).
    pub git_remote: Option<String>,
    /// Current branch (untrusted).
    pub branch: Option<String>,
    /// Current commit SHA (untrusted).
    pub commit_sha: Option<String>,
    /// Working directory (untrusted, not used as server path).
    pub working_directory: Option<String>,
    /// Client name and version (untrusted).
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    /// Provider conversation/session ID (untrusted, source locator not identity).
    pub provider_session_id: Option<String>,
    /// Explicit scope handle from a previous session.
    pub previous_handle: Option<String>,
    /// Task description (untrusted).
    pub task: Option<String>,
}

/// Bootstrap response returned to MCP clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapResponse {
    /// Opaque scope handle for subsequent operations.
    pub scope_handle: String,
    /// Resolved scope summary.
    pub resolved_scope: ResolvedScope,
    /// Capture limitations known for the caller/adapter.
    pub capture_mode: CaptureMode,
    pub channels: ChannelCompleteness,
    /// Compact Layer 0/1 context.
    pub context: CompactContext,
    /// Pending candidate counts.
    pub pending_candidates: i64,
    /// Freshness warnings.
    pub freshness_warnings: Vec<String>,
}

/// Compact context for bootstrap response (Layer 0/1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactContext {
    pub project_name: Option<String>,
    pub repository: Option<String>,
    pub purpose: Option<String>,
    pub architecture_summary: Option<String>,
    pub key_decisions: Vec<String>,
    pub active_constraints: Vec<String>,
    pub estimated_tokens: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::OwnershipDomain;

    #[test]
    fn mcp_only_capture_never_reports_complete() {
        let mode = CaptureMode::McpOnly;
        assert!(!mode.is_definitively_complete());
        let channels = ChannelCompleteness::mcp_only();
        assert!(!channels.all_complete());
        assert_eq!(channels.completed_count(), 0);
    }

    #[test]
    fn native_complete_is_definitively_complete() {
        assert!(CaptureMode::NativeComplete.is_definitively_complete());
        assert!(!CaptureMode::PartialAdapter.is_definitively_complete());
        assert!(!CaptureMode::Imported.is_definitively_complete());
        assert!(!CaptureMode::Unknown.is_definitively_complete());
    }

    #[test]
    fn scope_handle_usability() {
        let now = Utc::now();
        let mut handle = ScopeHandle {
            id: ScopeHandleId::generate(),
            tenant_id: TenantId::generate(),
            ownership_domain: OwnershipDomain::Personal,
            project_id: None,
            repository_id: None,
            workspace_id: None,
            scope_kind: ScopeKind::Project,
            resolution_state: ResolutionState::ProvisionalScope,
            label: None,
            created_at: now,
            expires_at: None,
            revoked_at: None,
        };
        assert!(handle.is_usable(now));

        handle.revoked_at = Some(now);
        assert!(!handle.is_usable(now));

        handle.revoked_at = None;
        handle.expires_at = Some(now - chrono::Duration::seconds(1));
        assert!(!handle.is_usable(now));

        handle.expires_at = Some(now + chrono::Duration::hours(1));
        assert!(handle.is_usable(now));
    }

    #[test]
    fn channel_completeness_count() {
        let mut c = ChannelCompleteness::mcp_only();
        assert_eq!(c.completed_count(), 0);
        c.user_messages = true;
        c.assistant_messages = true;
        assert_eq!(c.completed_count(), 2);
        assert!(!c.all_complete());
    }

    #[test]
    fn repository_provider_as_str() {
        assert_eq!(RepositoryProvider::GitHub.as_str(), "GITHUB");
        assert_eq!(RepositoryProvider::Unknown.as_str(), "UNKNOWN");
    }
}
