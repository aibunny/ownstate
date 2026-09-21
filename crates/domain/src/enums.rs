//! Closed vocabularies of the domain, stored as SCREAMING_SNAKE_CASE text in
//! PostgreSQL (with CHECK constraints) and validated here on the way in/out.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

macro_rules! string_enum {
    (
        $(#[$doc:meta])*
        $name:ident { $($variant:ident => $text:literal),+ $(,)? }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name {
            $(
                #[serde(rename = $text)]
                $variant,
            )+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub const fn as_str(&self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = DomainError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok($name::$variant),)+
                    other => Err(DomainError::InvalidEnumValue {
                        r#type: stringify!($name),
                        value: other.to_string(),
                    }),
                }
            }
        }
    };
}

string_enum!(
    /// Provider-neutral interaction event types (raw evidence layer).
    InteractionEventType {
        UserMessage => "USER_MESSAGE",
        AssistantMessage => "ASSISTANT_MESSAGE",
        ToolCall => "TOOL_CALL",
        ToolResult => "TOOL_RESULT",
        Command => "COMMAND",
        CommandResult => "COMMAND_RESULT",
        FileRead => "FILE_READ",
        FileWrite => "FILE_WRITE",
        Search => "SEARCH",
        SearchResult => "SEARCH_RESULT",
        DocumentRead => "DOCUMENT_READ",
        ArtifactCreated => "ARTIFACT_CREATED",
        GitDiff => "GIT_DIFF",
        TestResult => "TEST_RESULT",
        AgentResult => "AGENT_RESULT",
        SystemEvent => "SYSTEM_EVENT",
    }
);

string_enum!(
    /// Who produced an interaction event.
    ActorType {
        User => "USER",
        Assistant => "ASSISTANT",
        Tool => "TOOL",
        System => "SYSTEM",
    }
);

string_enum!(
    /// Kinds of durable knowledge. The storage layer keeps this open for
    /// future kinds (TEXT + CHECK, extended by migration).
    KnowledgeKind {
        Fact => "FACT",
        Architecture => "ARCHITECTURE",
        Decision => "DECISION",
        Rationale => "RATIONALE",
        Constraint => "CONSTRAINT",
        Requirement => "REQUIREMENT",
        Procedure => "PROCEDURE",
        Preference => "PREFERENCE",
        Failure => "FAILURE",
        Outcome => "OUTCOME",
        Goal => "GOAL",
        Risk => "RISK",
        Relationship => "RELATIONSHIP",
        Definition => "DEFINITION",
        OpenQuestion => "OPEN_QUESTION",
    }
);

string_enum!(
    /// Lifecycle status of a knowledge version. Only ACTIVE versions are
    /// eligible for retrieval and context compilation.
    KnowledgeStatus {
        Candidate => "CANDIDATE",
        Active => "ACTIVE",
        Stale => "STALE",
        Superseded => "SUPERSEDED",
        Conflict => "CONFLICT",
        Quarantined => "QUARANTINED",
        Revoked => "REVOKED",
    }
);

string_enum!(
    /// How much the system trusts a knowledge version. Distinct from model
    /// confidence: trust reflects the provenance path, not the extractor's
    /// self-assessment.
    TrustLevel {
        HumanExplicit => "HUMAN_EXPLICIT",
        SourceVerified => "SOURCE_VERIFIED",
        RepoVerified => "REPO_VERIFIED",
        AgentDerived => "AGENT_DERIVED",
        ExternalVerified => "EXTERNAL_VERIFIED",
        ExternalUntrusted => "EXTERNAL_UNTRUSTED",
    }
);

impl TrustLevel {
    /// Deterministic multiplier applied to fused retrieval scores so that
    /// better-provenanced knowledge outranks equally relevant but less
    /// trusted knowledge.
    pub const fn rank_weight(&self) -> f64 {
        match self {
            TrustLevel::HumanExplicit => 1.20,
            TrustLevel::SourceVerified => 1.15,
            TrustLevel::RepoVerified => 1.10,
            TrustLevel::AgentDerived => 1.00,
            TrustLevel::ExternalVerified => 0.90,
            TrustLevel::ExternalUntrusted => 0.70,
        }
    }
}

string_enum!(
    /// Who owns a body of knowledge. Personal knowledge never silently
    /// becomes organizational and vice versa.
    OwnershipDomain {
        Personal => "PERSONAL",
        Organization => "ORGANIZATION",
    }
);

string_enum!(
    /// Data classification. Ordered: PUBLIC < INTERNAL < CONFIDENTIAL <
    /// RESTRICTED < SECRET (derived `Ord` follows declaration order).
    SecurityClassification {
        Public => "PUBLIC",
        Internal => "INTERNAL",
        Confidential => "CONFIDENTIAL",
        Restricted => "RESTRICTED",
        Secret => "SECRET",
    }
);

impl SecurityClassification {
    /// Derived content may be more restricted than its cited evidence, never
    /// less. Missing request classification retains the INTERNAL default.
    /// Declassification requires a separate policy capability, not a proposal.
    pub fn with_evidence_floor(
        requested: Option<Self>,
        sources: impl IntoIterator<Item = Self>,
    ) -> Self {
        sources
            .into_iter()
            .fold(requested.unwrap_or(Self::Internal), Self::max)
    }

    /// All classifications at or below `ceiling`, as stored text values.
    /// Used to scope retrieval before results leave the database.
    pub fn allowed_up_to(ceiling: SecurityClassification) -> Vec<&'static str> {
        SecurityClassification::ALL
            .iter()
            .filter(|c| **c <= ceiling)
            .map(|c| c.as_str())
            .collect()
    }
}

string_enum!(
    /// Lifecycle of candidate knowledge (the untrusted proposal layer).
    CandidateStatus {
        Pending => "PENDING",
        Promoted => "PROMOTED",
        Rejected => "REJECTED",
    }
);

string_enum!(
    /// Who proposed a candidate. Drives deterministic trust assignment;
    /// extractor output can never claim HUMAN_EXPLICIT trust.
    ProposerKind {
        Human => "HUMAN",
        Agent => "AGENT",
        Extractor => "EXTRACTOR",
    }
);

string_enum!(
    SessionStatus {
        Active => "ACTIVE",
        Completed => "COMPLETED",
        Abandoned => "ABANDONED",
    }
);

string_enum!(
    /// Background job kinds. PostgreSQL is the durable queue.
    JobKind {
        GenerateEmbedding => "GENERATE_EMBEDDING",
    }
);

string_enum!(
    JobStatus {
        Pending => "PENDING",
        Running => "RUNNING",
        Succeeded => "SUCCEEDED",
        Failed => "FAILED",
    }
);

string_enum!(
    /// How complete the observable capture is for a session/source.
    /// MCP-only observation can never be FULL; adapters mark their actual
    /// channel coverage.
    CaptureMode {
        NativeComplete => "NATIVE_COMPLETE",
        PartialAdapter => "PARTIAL_ADAPTER",
        McpOnly => "MCP_ONLY",
        Imported => "IMPORTED",
        Unknown => "UNKNOWN",
    }
);

impl CaptureMode {
    /// MCP-only observation never implies complete capture.
    pub const fn is_definitively_complete(&self) -> bool {
        matches!(self, Self::NativeComplete)
    }
}

string_enum!(
    /// The kind of institutional scope a session or handle references.
    /// A session may reference multiple scopes.
    ScopeKind {
        Organization => "ORGANIZATION",
        Project => "PROJECT",
        Repository => "REPOSITORY",
        Relationship => "RELATIONSHIP",
        Artifact => "ARTIFACT",
        Thread => "THREAD",
        Personal => "PERSONAL",
    }
);

string_enum!(
    /// How deterministically the resolver identified a scope.
    ResolutionState {
        ExactStable => "EXACT_STABLE",
        KnownAlias => "KNOWN_ALIAS",
        ProbableAssociation => "PROBABLE_ASSOCIATION",
        ProvisionalScope => "PROVISIONAL_SCOPE",
        GeneralScope => "GENERAL_SCOPE",
    }
);

/// Per-channel capture completeness. Each channel is independently tracked;
/// completeness is factual metadata, not a confidence guess.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelCompleteness {
    pub user_messages: bool,
    pub assistant_messages: bool,
    pub tool_calls: bool,
    pub tool_results: bool,
    pub commands: bool,
    pub command_results: bool,
    pub file_reads: bool,
    pub file_writes: bool,
    pub diffs: bool,
    pub tests: bool,
    pub artifacts: bool,
    pub lifecycle: bool,
}

impl ChannelCompleteness {
    /// MCP-only observation: nothing is provably complete.
    pub const fn mcp_only() -> Self {
        Self {
            user_messages: false,
            assistant_messages: false,
            tool_calls: false,
            tool_results: false,
            commands: false,
            command_results: false,
            file_reads: false,
            file_writes: false,
            diffs: false,
            tests: false,
            artifacts: false,
            lifecycle: false,
        }
    }

    /// True when every channel is marked complete.
    pub const fn all_complete(&self) -> bool {
        self.user_messages
            && self.assistant_messages
            && self.tool_calls
            && self.tool_results
            && self.commands
            && self.command_results
            && self.file_reads
            && self.file_writes
            && self.diffs
            && self.tests
            && self.artifacts
            && self.lifecycle
    }

    /// Count of channels marked complete.
    pub fn completed_count(&self) -> u32 {
        let mut n = 0;
        if self.user_messages {
            n += 1;
        }
        if self.assistant_messages {
            n += 1;
        }
        if self.tool_calls {
            n += 1;
        }
        if self.tool_results {
            n += 1;
        }
        if self.commands {
            n += 1;
        }
        if self.command_results {
            n += 1;
        }
        if self.file_reads {
            n += 1;
        }
        if self.file_writes {
            n += 1;
        }
        if self.diffs {
            n += 1;
        }
        if self.tests {
            n += 1;
        }
        if self.artifacts {
            n += 1;
        }
        if self.lifecycle {
            n += 1;
        }
        n
    }

    pub const TOTAL_CHANNELS: u32 = 12;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn classification_is_ordered() {
        assert!(SecurityClassification::Public < SecurityClassification::Internal);
        assert!(SecurityClassification::Confidential < SecurityClassification::Restricted);
        assert!(SecurityClassification::Restricted < SecurityClassification::Secret);
        assert_eq!(
            SecurityClassification::allowed_up_to(SecurityClassification::Internal),
            vec!["PUBLIC", "INTERNAL"]
        );
    }

    #[test]
    fn derived_classification_never_downgrades_a_source_or_request() {
        for requested in SecurityClassification::ALL {
            for source in SecurityClassification::ALL {
                let derived = SecurityClassification::with_evidence_floor(
                    Some(*requested),
                    [SecurityClassification::Public, *source],
                );
                assert!(derived >= *source);
                assert!(derived >= *requested);
                assert_eq!(derived, (*requested).max(*source));
            }
        }
        assert_eq!(
            SecurityClassification::with_evidence_floor(None, []),
            SecurityClassification::Internal
        );
        assert_eq!(
            SecurityClassification::with_evidence_floor(
                None,
                [
                    SecurityClassification::Secret,
                    SecurityClassification::Internal
                ],
            ),
            SecurityClassification::Secret
        );
    }

    #[test]
    fn enums_round_trip_through_text() {
        for kind in KnowledgeKind::ALL {
            assert_eq!(&kind.as_str().parse::<KnowledgeKind>().unwrap(), kind);
        }
        for status in KnowledgeStatus::ALL {
            assert_eq!(&status.as_str().parse::<KnowledgeStatus>().unwrap(), status);
        }
        for t in TrustLevel::ALL {
            assert_eq!(&t.as_str().parse::<TrustLevel>().unwrap(), t);
        }
        assert!("NOT_A_KIND".parse::<KnowledgeKind>().is_err());
    }

    #[test]
    fn trust_weights_are_monotonic() {
        assert!(TrustLevel::HumanExplicit.rank_weight() > TrustLevel::AgentDerived.rank_weight());
        assert!(
            TrustLevel::AgentDerived.rank_weight() > TrustLevel::ExternalUntrusted.rank_weight()
        );
    }
}
