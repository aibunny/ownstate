//! Strongly typed identifiers. UUIDv7 so ids sort by creation time.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new time-ordered (UUIDv7) identifier.
            pub fn generate() -> Self {
                Self(Uuid::now_v7())
            }

            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::from_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id!(
    /// Owning tenant. Single fixed tenant in personal mode; real tenancy later.
    TenantId
);
define_id!(ProjectId);
define_id!(SessionId);
define_id!(EventId);
define_id!(CandidateId);
define_id!(KnowledgeItemId);
define_id!(KnowledgeVersionId);
define_id!(EvidenceId);
define_id!(EmbeddingId);
define_id!(ContextPacketId);
define_id!(JobId);
define_id!(EntityId);
define_id!(GraphCandidateId);
define_id!(AssertionId);
define_id!(AssertionVersionId);
define_id!(UserId);
define_id!(OrganizationId);
define_id!(WorkspaceId);
define_id!(RepositoryId);
define_id!(PrincipalId);
define_id!(CredentialId);
define_id!(GrantId);
define_id!(AgentExecutionId);
define_id!(PolicyDecisionId);
define_id!(ScopeHandleId);
define_id!(CaptureAssessmentId);
