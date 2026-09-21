//! Closed authorization vocabulary and pure attenuation rules.
use crate::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrincipalKind {
    User,
    Service,
    Agent,
}
impl PrincipalKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::Service => "SERVICE",
            Self::Agent => "AGENT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyAction {
    ReadProject,
    ReadEvidence,
    ReadKnowledge,
    CompileContext,
    CreateProject,
    CreateSession,
    AppendEvidence,
    ProposeKnowledge,
    AttributeHumanProposal,
    PromoteKnowledge,
    RejectCandidate,
    GenerateEmbedding,
    ModelEgress,
    DelegateAgent,
}
impl PolicyAction {
    pub const ALL: &'static [Self] = &[
        Self::ReadProject,
        Self::ReadEvidence,
        Self::ReadKnowledge,
        Self::CompileContext,
        Self::CreateProject,
        Self::CreateSession,
        Self::AppendEvidence,
        Self::ProposeKnowledge,
        Self::AttributeHumanProposal,
        Self::PromoteKnowledge,
        Self::RejectCandidate,
        Self::GenerateEmbedding,
        Self::ModelEgress,
        Self::DelegateAgent,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadProject => "READ_PROJECT",
            Self::ReadEvidence => "READ_EVIDENCE",
            Self::ReadKnowledge => "READ_KNOWLEDGE",
            Self::CompileContext => "COMPILE_CONTEXT",
            Self::CreateProject => "CREATE_PROJECT",
            Self::CreateSession => "CREATE_SESSION",
            Self::AppendEvidence => "APPEND_EVIDENCE",
            Self::ProposeKnowledge => "PROPOSE_KNOWLEDGE",
            Self::AttributeHumanProposal => "ATTRIBUTE_HUMAN_PROPOSAL",
            Self::PromoteKnowledge => "PROMOTE_KNOWLEDGE",
            Self::RejectCandidate => "REJECT_CANDIDATE",
            Self::GenerateEmbedding => "GENERATE_EMBEDDING",
            Self::ModelEgress => "MODEL_EGRESS",
            Self::DelegateAgent => "DELEGATE_AGENT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceScope {
    pub tenant_id: TenantId,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelDestination {
    Local(String),
    Remote(String),
}
impl ModelDestination {
    pub fn policy_key(&self) -> String {
        match self {
            Self::Local(v) => format!("LOCAL:{v}"),
            Self::Remote(v) => format!("REMOTE:{v}"),
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionBudget {
    pub max_tokens: Option<i64>,
    pub max_items: Option<i32>,
}
#[derive(Debug, Clone)]
pub struct PolicyRequest {
    pub action: PolicyAction,
    pub resource: ResourceScope,
    pub classification: SecurityClassification,
    pub destination: Option<ModelDestination>,
    pub budget: ExecutionBudget,
    pub now: DateTime<Utc>,
    pub execution_id: Option<AgentExecutionId>,
}
#[derive(Debug, Clone)]
pub struct AuthorizedScope {
    grant_id: GrantId,
    pub resource: ResourceScope,
    pub classification_ceiling: SecurityClassification,
    pub destination: Option<String>,
    pub budget: ExecutionBudget,
}
impl AuthorizedScope {
    #[doc(hidden)]
    pub fn from_policy(
        grant_id: GrantId,
        resource: ResourceScope,
        classification_ceiling: SecurityClassification,
        destination: Option<String>,
        budget: ExecutionBudget,
    ) -> Self {
        Self {
            grant_id,
            resource,
            classification_ceiling,
            destination,
            budget,
        }
    }
    pub const fn grant_id(&self) -> GrantId {
        self.grant_id
    }
}

#[derive(Debug, Clone)]
pub struct GrantSpec {
    pub tenant_id: TenantId,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub actions: Vec<PolicyAction>,
    pub delegable_actions: Vec<PolicyAction>,
    pub classification_ceiling: SecurityClassification,
    pub allowed_destinations: Vec<String>,
    pub budget: ExecutionBudget,
    pub valid_until: Option<DateTime<Utc>>,
}
impl GrantSpec {
    pub fn validate(&self) -> Result<(), String> {
        if self.actions.is_empty()
            || self.actions.len() > PolicyAction::ALL.len()
            || self
                .delegable_actions
                .iter()
                .any(|a| !self.actions.contains(a))
        {
            return Err("grant actions are empty, excessive, or not delegable subsets".into());
        }
        if self.budget.max_tokens.is_some_and(|v| v <= 0)
            || self.budget.max_items.is_some_and(|v| v <= 0)
        {
            return Err("grant budgets must be positive".into());
        }
        if self.allowed_destinations.len() > 32
            || self
                .allowed_destinations
                .iter()
                .any(|v| v.trim() != v || v.is_empty() || v.len() > 200)
        {
            return Err("invalid destinations".into());
        }
        Ok(())
    }
    pub fn attenuates(&self, parent: &Self) -> Result<(), String> {
        self.validate()?;
        parent.validate()?;
        if self.tenant_id != parent.tenant_id
            || self.workspace_id != parent.workspace_id
            || parent.project_id.is_some() && self.project_id != parent.project_id
            || self
                .actions
                .iter()
                .any(|a| !parent.delegable_actions.contains(a))
            || self
                .delegable_actions
                .iter()
                .any(|a| !parent.delegable_actions.contains(a))
            || self.classification_ceiling > parent.classification_ceiling
            || self
                .allowed_destinations
                .iter()
                .any(|d| !parent.allowed_destinations.contains(d))
            || exceeds(self.budget.max_tokens, parent.budget.max_tokens)
            || exceeds(self.budget.max_items, parent.budget.max_items)
            || later(self.valid_until, parent.valid_until)
        {
            return Err("child grant exceeds parent delegation".into());
        }
        Ok(())
    }
}
fn exceeds<T: PartialOrd>(child: Option<T>, parent: Option<T>) -> bool {
    match (child, parent) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(c), Some(p)) => c > p,
    }
}
fn later(child: Option<DateTime<Utc>>, parent: Option<DateTime<Utc>>) -> bool {
    match (child, parent) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(c), Some(p)) => c > p,
    }
}
impl std::str::FromStr for PrincipalKind {
    type Err = DomainError;
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        match v {
            "USER" => Ok(Self::User),
            "SERVICE" => Ok(Self::Service),
            "AGENT" => Ok(Self::Agent),
            _ => Err(DomainError::InvalidEnumValue {
                r#type: "PrincipalKind",
                value: v.into(),
            }),
        }
    }
}
impl std::str::FromStr for PolicyAction {
    type Err = DomainError;
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|a| a.as_str() == v)
            .ok_or_else(|| DomainError::InvalidEnumValue {
                r#type: "PolicyAction",
                value: v.into(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_never_inherits() {
        let t = TenantId::generate();
        let w = WorkspaceId::generate();
        let p = GrantSpec {
            tenant_id: t,
            workspace_id: w,
            project_id: None,
            actions: vec![PolicyAction::ReadKnowledge, PolicyAction::DelegateAgent],
            delegable_actions: vec![PolicyAction::ReadKnowledge],
            classification_ceiling: SecurityClassification::Internal,
            allowed_destinations: vec![],
            budget: ExecutionBudget {
                max_tokens: Some(100),
                max_items: Some(3),
            },
            valid_until: None,
        };
        let mut c = p.clone();
        c.actions = vec![PolicyAction::ReadKnowledge];
        c.delegable_actions = vec![];
        assert!(c.attenuates(&p).is_ok());
        c.actions.push(PolicyAction::PromoteKnowledge);
        assert!(c.attenuates(&p).is_err())
    }
}
