use chrono::Utc;
use ownstate_domain::{OwnershipDomain, Project, ProjectId, limits};
use ownstate_storage::{policy, projects};
use serde_json::Value as Json;

use crate::{AppServices, ServiceError, ServiceResult};

pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub ownership_domain: Option<OwnershipDomain>,
    pub metadata: Option<Json>,
}

/// Workspace-driven project resolution. Identity is multi-factor so the same
/// project reached from different clients, clones or directory names does not
/// duplicate: git origin (strongest — same repository ⇒ same project), then
/// recorded filesystem location, then case-insensitive name.
pub struct EnsureProject {
    pub name: String,
    pub git_origin: Option<String>,
    /// Absolute path of the workspace directory, recorded as
    /// `metadata.root_path` and used as an identity factor.
    pub root_path: Option<String>,
}

impl AppServices {
    #[allow(dead_code)]
    pub(crate) async fn create_project(&self, req: CreateProject) -> ServiceResult<Project> {
        let name = req.name.trim().to_string();
        if name.is_empty() || name.len() > limits::MAX_NAME_LEN {
            return Err(ServiceError::validation(format!(
                "project name must be 1..={} characters",
                limits::MAX_NAME_LEN
            )));
        }

        let project = Project {
            id: ProjectId::generate(),
            tenant_id: self.tenant_id(),
            ownership_domain: req.ownership_domain.unwrap_or(OwnershipDomain::Personal),
            name,
            description: req.description,
            metadata: req
                .metadata
                .unwrap_or_else(|| Json::Object(Default::default())),
            created_at: Utc::now(),
        };
        let bootstrap = policy::bootstrap_personal_policy(self.pool(), self.tenant_id()).await?;
        let mut tx = self.pool().begin().await?;
        match projects::insert(&mut *tx, &project).await {
            Ok(()) => {}
            Err(e) if e.is_unique_violation() => {
                return Err(crate::ServiceError::Conflict(format!(
                    "a project named '{}' already exists",
                    project.name
                )));
            }
            Err(e) => return Err(e.into()),
        }
        if project.ownership_domain == OwnershipDomain::Personal {
            policy::bind_project(
                &mut *tx,
                project.tenant_id,
                project.id,
                bootstrap.workspace_id,
                None,
                OwnershipDomain::Personal,
            )
            .await?;
        }
        tx.commit().await?;
        tracing::info!(project_id = %project.id, "project created");
        Ok(project)
    }

    pub(crate) async fn get_project(&self, id: ProjectId) -> ServiceResult<Project> {
        Ok(projects::get(self.pool(), self.tenant_id(), id).await?)
    }

    /// Get-or-create a project for a workspace. Resolution order:
    ///
    /// 1. **git origin** — a project already recording this origin IS this
    ///    project, whatever its name or where it is cloned now.
    /// 2. **root path** — a project recorded at this filesystem location
    ///    (covers repos whose remote was added/renamed later).
    /// 3. **name** (case-insensitive) — but only when its identity is
    ///    compatible: if the stored project points at a DIFFERENT git origin,
    ///    this is a different project that happens to share a directory name,
    ///    and a distinct, deterministically-suffixed project is created
    ///    instead of silently merging knowledge across repositories.
    /// 4. Create, recording name, git origin and root path.
    ///
    /// Race-safe: a concurrent creator's insert wins via the unique
    /// (tenant, lower(name)) index and we fall back to reading their row.
    pub(crate) async fn ensure_project(&self, req: EnsureProject) -> ServiceResult<Project> {
        let name = req.name.trim().to_string();
        if name.is_empty() || name.len() > limits::MAX_NAME_LEN {
            return Err(ServiceError::validation(format!(
                "project name must be 1..={} characters",
                limits::MAX_NAME_LEN
            )));
        }
        if let Some(path) = &req.root_path
            && path.len() > 1000
        {
            return Err(ServiceError::validation(
                "root_path exceeds 1000 characters",
            ));
        }

        // Factor 1: same repository ⇒ same project.
        if let Some(origin) = &req.git_origin
            && let Some(existing) =
                projects::find_by_git_origin(self.pool(), self.tenant_id(), origin).await?
        {
            return self.record_workspace(existing, &req).await;
        }

        // Factor 2: same recorded location ⇒ same project.
        if let Some(path) = &req.root_path
            && let Some(existing) =
                projects::find_by_root_path(self.pool(), self.tenant_id(), path).await?
        {
            return self.record_workspace(existing, &req).await;
        }

        // Factor 3: name — only when the stored identity is compatible.
        let mut create_name = name.clone();
        if let Some(existing) = projects::find_by_name(self.pool(), self.tenant_id(), &name).await?
        {
            let stored_origin = existing.metadata.get("git_origin").and_then(|v| v.as_str());
            match (&req.git_origin, stored_origin) {
                // Different repositories sharing a directory name: distinct
                // projects. Derive a stable suffixed name from the origin.
                (Some(detected), Some(stored)) if stored != detected => {
                    create_name = suffixed_name(&name, detected);
                    if let Some(suffixed) =
                        projects::find_by_name(self.pool(), self.tenant_id(), &create_name).await?
                    {
                        return self.record_workspace(suffixed, &req).await;
                    }
                    tracing::warn!(
                        name = %name,
                        "directory name collides with a project of a different git \
                         origin; creating a distinct project"
                    );
                }
                _ => return self.record_workspace(existing, &req).await,
            }
        }

        let mut metadata = serde_json::Map::new();
        if let Some(origin) = &req.git_origin {
            metadata.insert("git_origin".into(), Json::String(origin.clone()));
        }
        if let Some(path) = &req.root_path {
            metadata.insert("root_path".into(), Json::String(path.clone()));
        }
        let project = Project {
            id: ProjectId::generate(),
            tenant_id: self.tenant_id(),
            ownership_domain: OwnershipDomain::Personal,
            name: create_name.clone(),
            description: None,
            metadata: Json::Object(metadata),
            created_at: Utc::now(),
        };
        let bootstrap = policy::bootstrap_personal_policy(self.pool(), self.tenant_id()).await?;
        let mut tx = self.pool().begin().await?;
        match projects::insert(&mut *tx, &project).await {
            Ok(()) => {
                policy::bind_project(
                    &mut *tx,
                    project.tenant_id,
                    project.id,
                    bootstrap.workspace_id,
                    None,
                    OwnershipDomain::Personal,
                )
                .await?;
                tx.commit().await?;
                tracing::info!(
                    project_id = %project.id,
                    name = %project.name,
                    has_git_origin = req.git_origin.is_some(),
                    has_root_path = req.root_path.is_some(),
                    "project created from workspace"
                );
                Ok(project)
            }
            Err(e) if e.is_unique_violation() => {
                tx.rollback().await?;
                // Lost the creation race; the winner's row is the identity.
                let existing = projects::find_by_name(self.pool(), self.tenant_id(), &create_name)
                    .await?
                    .ok_or(ServiceError::NotFound("project"))?;
                self.record_workspace(existing, &req).await
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Merge detected workspace identity into project metadata: backfill a
    /// missing git origin, keep `root_path` pointing at where the workspace
    /// was last seen, and update the origin when the project was matched by
    /// location (a repository's remote can legitimately change). A stored
    /// origin is never clobbered by a conflicting one on a name match — that
    /// case creates a separate project upstream.
    async fn record_workspace(
        &self,
        mut project: Project,
        req: &EnsureProject,
    ) -> ServiceResult<Project> {
        let stored_origin = project
            .metadata
            .get("git_origin")
            .and_then(|v| v.as_str())
            .map(String::from);
        let stored_path = project
            .metadata
            .get("root_path")
            .and_then(|v| v.as_str())
            .map(String::from);

        let matched_by_path = req.root_path.is_some() && stored_path == req.root_path;
        let mut changed = false;
        let map = match &mut project.metadata {
            Json::Object(map) => map,
            other => {
                *other = Json::Object(serde_json::Map::new());
                match other {
                    Json::Object(map) => map,
                    // Just assigned an object above.
                    _ => unreachable!(),
                }
            }
        };

        if let Some(detected) = &req.git_origin {
            match &stored_origin {
                None => {
                    map.insert("git_origin".into(), Json::String(detected.clone()));
                    changed = true;
                }
                Some(stored) if stored != detected && matched_by_path => {
                    tracing::info!(project_id = %project.id, "git origin updated (remote changed)");
                    map.insert("git_origin".into(), Json::String(detected.clone()));
                    changed = true;
                }
                _ => {}
            }
        }
        if let Some(path) = &req.root_path
            && stored_path.as_deref() != Some(path)
        {
            map.insert("root_path".into(), Json::String(path.clone()));
            changed = true;
        }

        if changed {
            projects::update_metadata(self.pool(), self.tenant_id(), project.id, &project.metadata)
                .await?;
            tracing::info!(project_id = %project.id, "workspace identity recorded on project");
        }
        Ok(project)
    }
}

/// Deterministic distinct name for a same-named workspace backed by a
/// different repository: base name plus a short digest of the origin.
fn suffixed_name(name: &str, origin: &str) -> String {
    let digest = ownstate_domain::content_hash(origin.as_bytes());
    let short = &digest["blake3:".len().."blake3:".len() + 8];
    let base: String = name.chars().take(limits::MAX_NAME_LEN - 9).collect();
    format!("{base}-{short}")
}
