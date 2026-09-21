//! Static, scoped SQL for institutional candidates and immutable versions.
use chrono::{DateTime, Utc};
use ownstate_domain::*;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{Result, StorageError};

fn json_error(error: serde_json::Error) -> StorageError {
    StorageError::Sqlx(sqlx::Error::Decode(Box::new(error)))
}

pub async fn insert_candidate<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    c: &GraphCandidate,
) -> Result<()> {
    sqlx::query("INSERT INTO institutional_candidates
        (id,tenant_id,project_id,proposal,evidence_event_ids,security_classification,proposed_by,status,observed_at,created_at,confidence)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(c.id.as_uuid()).bind(c.tenant_id.as_uuid()).bind(c.project_id.as_uuid())
        .bind(serde_json::to_value(&c.proposal).map_err(json_error)?)
        .bind(c.evidence_event_ids.iter().map(EventId::as_uuid).collect::<Vec<_>>())
        .bind(c.security_classification.as_str()).bind(c.proposed_by.as_str())
        .bind(c.status.as_str()).bind(c.observed_at).bind(c.created_at).bind(c.confidence).execute(exec).await?;
    Ok(())
}

pub async fn lock_candidate(
    tx: &mut Transaction<'_, Postgres>,
    tenant: TenantId,
    id: GraphCandidateId,
) -> Result<GraphCandidate> {
    let row: Option<(Value,)> = sqlx::query_as(
        "SELECT to_jsonb(c) FROM institutional_candidates c
        WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
    )
    .bind(tenant.as_uuid())
    .bind(id.as_uuid())
    .fetch_optional(&mut **tx)
    .await?;
    serde_json::from_value(
        row.ok_or_else(|| StorageError::not_found("institutional candidate"))?
            .0,
    )
    .map_err(json_error)
}

/// All visible versions at valid time; transaction time remains in each result.
/// Edges are withheld when either endpoint is unavailable at the same ceiling/time.
pub struct InstitutionalRead<'a> {
    pub tenant: TenantId,
    pub project: ProjectId,
    pub record_id: Option<Uuid>,
    pub allowed: &'a [&'a str],
    pub as_of: DateTime<Utc>,
    pub record_type: Option<&'a str>,
    pub endpoint: Option<EntityId>,
    pub limit: i64,
}

pub async fn versions<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: InstitutionalRead<'_>,
) -> Result<Vec<InstitutionalVersion>> {
    let InstitutionalRead {
        tenant,
        project,
        record_id,
        allowed,
        as_of,
        record_type,
        endpoint,
        limit,
    } = q;
    let rows: Vec<(Value,)> = sqlx::query_as("SELECT v.data FROM institutional_version_data v
        WHERE v.tenant_id=$1 AND v.project_id=$2 AND ($3::uuid IS NULL OR v.record_id=$3)
        AND v.security_classification=ANY($4) AND v.valid_from<=$5
        AND (v.valid_until IS NULL OR v.valid_until>$5) AND v.status IN ('ACTIVE','SUPERSEDED')
        AND ($6::text IS NULL OR v.record_type=$6)
        AND ($8::uuid IS NULL OR v.subject_id=$8 OR v.target_id=$8)
        AND (v.subject_id IS NULL OR EXISTS (SELECT 1 FROM institutional_version_data s
            WHERE s.record_id=v.subject_id AND s.record_type='ENTITY' AND s.security_classification=ANY($4)
            AND s.status IN ('ACTIVE','SUPERSEDED') AND s.valid_from<=$5 AND (s.valid_until IS NULL OR s.valid_until>$5)))
        AND (v.target_id IS NULL OR EXISTS (SELECT 1 FROM institutional_version_data t
            WHERE t.record_id=v.target_id AND t.record_type='ENTITY' AND t.security_classification=ANY($4)
            AND t.status IN ('ACTIVE','SUPERSEDED') AND t.valid_from<=$5 AND (t.valid_until IS NULL OR t.valid_until>$5)))
        ORDER BY v.recorded_at,v.id LIMIT $7")
        .bind(tenant.as_uuid()).bind(project.as_uuid()).bind(record_id).bind(allowed)
        .bind(as_of).bind(record_type).bind(limit).bind(endpoint.map(|id|id.as_uuid())).fetch_all(exec).await?;
    rows.into_iter()
        .map(|(data,)| serde_json::from_value(data).map_err(json_error))
        .collect()
}

pub async fn history<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    project: ProjectId,
    record: Uuid,
    allowed: &[&str],
) -> Result<Vec<InstitutionalVersion>> {
    let rows: Vec<(Value,)> = sqlx::query_as(
        "SELECT v.data FROM institutional_version_data v
        WHERE v.tenant_id=$1 AND v.project_id=$2 AND v.record_id=$3 AND v.security_classification=ANY($4)
        AND v.status NOT IN ('QUARANTINED','REVOKED')
        AND (v.subject_id IS NULL OR EXISTS (SELECT 1 FROM institutional_version_data s
            WHERE s.record_id=v.subject_id AND s.record_type='ENTITY' AND s.security_classification=ANY($4)
            AND s.status='ACTIVE' AND s.valid_from<=statement_timestamp()
            AND (s.valid_until IS NULL OR s.valid_until>statement_timestamp())))
        AND (v.target_id IS NULL OR EXISTS (SELECT 1 FROM institutional_version_data t
            WHERE t.record_id=v.target_id AND t.record_type='ENTITY' AND t.security_classification=ANY($4)
            AND t.status='ACTIVE' AND t.valid_from<=statement_timestamp()
            AND (t.valid_until IS NULL OR t.valid_until>statement_timestamp())))
        ORDER BY v.version_number",
    )
    .bind(tenant.as_uuid())
    .bind(project.as_uuid())
    .bind(record)
    .bind(allowed)
    .fetch_all(exec)
    .await?;
    rows.into_iter()
        .map(|(data,)| serde_json::from_value(data).map_err(json_error))
        .collect()
}

pub async fn resolve<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    project: ProjectId,
    name: &str,
    namespace: Option<&str>,
    allowed: &[&str],
    as_of: DateTime<Utc>,
) -> Result<EntityResolution> {
    let rows: Vec<(Uuid,Value)> = sqlx::query_as("SELECT record_id,data FROM institutional_version_data v
        WHERE tenant_id=$1 AND project_id=$2 AND record_type='ENTITY' AND security_classification=ANY($5)
        AND status IN ('ACTIVE','SUPERSEDED') AND valid_from<=$6 AND (valid_until IS NULL OR valid_until>$6)
        AND (($4::text IS NOT NULL AND proposal->'external_ids'->>$4=$3)
          OR ($4::text IS NULL AND (lower(proposal->>'name')=lower($3)
          OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(proposal->'aliases') alias WHERE lower(alias)=lower($3)))))
        ORDER BY record_id LIMIT 101")
        .bind(tenant.as_uuid()).bind(project.as_uuid()).bind(name).bind(namespace).bind(allowed).bind(as_of).fetch_all(exec).await?;
    let matches = rows
        .into_iter()
        .map(|(id, data)| {
            Ok(Entity {
                id: id.into(),
                version: serde_json::from_value(data).map_err(json_error)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(EntityResolution {
        ambiguous: matches.len() > 1,
        matches,
    })
}

/// Caller has validated evidence, scope, policy, and supplied deterministic trust.
/// The database repeats provenance validation and freezes canonical content.
pub async fn promote_candidate(
    tx: &mut Transaction<'_, Postgres>,
    c: &GraphCandidate,
    trust: TrustLevel,
    classification: SecurityClassification,
) -> Result<InstitutionalVersion> {
    if c.status != CandidateStatus::Pending {
        return Err(StorageError::Conflict("candidate is not pending".into()));
    }
    let (record_type, key, subject, target, existing_entity) = match &c.proposal {
        GraphProposal::Entity {
            kind, external_ids, ..
        } => {
            // Serialize promotions per project to fence concurrent exact identity
            // resolution, including first inserts where no entity row exists yet.
            sqlx::query("SELECT id FROM projects WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
                .bind(c.tenant_id.as_uuid())
                .bind(c.project_id.as_uuid())
                .execute(&mut **tx)
                .await?;
            let ids = serde_json::to_value(external_ids).map_err(json_error)?;
            let rows:Vec<(Uuid,Value,i32,String)> = sqlx::query_as("SELECT DISTINCT ON (r.id) r.id,v.proposal,
                (SELECT max(ownstate_classification_rank(h.security_classification)) FROM institutional_versions h WHERE h.record_id=r.id AND h.status<>'CONFLICT'),v.status
                FROM institutional_identifiers i JOIN institutional_records r ON r.id=i.entity_id
                JOIN institutional_versions v ON v.record_id=r.id
                WHERE i.tenant_id=$1 AND i.project_id=$2 AND r.record_type='ENTITY' AND v.status<>'CONFLICT'
                AND EXISTS (SELECT 1 FROM jsonb_each_text($3::jsonb) x WHERE i.namespace=x.key AND i.external_id=x.value)
                ORDER BY r.id,v.version_number DESC")
                .bind(c.tenant_id.as_uuid()).bind(c.project_id.as_uuid()).bind(ids).fetch_all(&mut **tx).await?;
            if rows.len() > 1 {
                return Err(StorageError::Conflict(
                    "external identifiers resolve to multiple entities".into(),
                ));
            }
            let found = if let Some((id, data, ceiling, status)) = rows.first() {
                if status == "REVOKED" || status == "QUARANTINED" {
                    return Err(StorageError::Conflict(
                        "identity requires explicit lifecycle review".into(),
                    ));
                }
                let rank = SecurityClassification::ALL
                    .iter()
                    .position(|c| *c == classification)
                    .ok_or_else(|| StorageError::Conflict("invalid classification".into()))?;
                if *ceiling > rank as i32 {
                    return Err(StorageError::Conflict(
                        "identity is unavailable at candidate classification".into(),
                    ));
                }
                let proposal: GraphProposal =
                    serde_json::from_value(data.clone()).map_err(json_error)?;
                if let GraphProposal::Entity {
                    kind: old_kind,
                    external_ids: old_ids,
                    ..
                } = proposal
                    && (old_kind != *kind
                        || external_ids
                            .iter()
                            .any(|(k, v)| old_ids.get(k).is_some_and(|old| old != v))
                        || old_ids.iter().any(|(k, v)| external_ids.get(k) != Some(v)))
                {
                    return Err(StorageError::Conflict(
                        "entity identifiers or kind contradict existing identity".into(),
                    ));
                }
                Some(*id)
            } else {
                None
            };
            let id = found.unwrap_or_else(|| EntityId::generate().as_uuid());
            ("ENTITY", id.to_string(), None, None, Some(id))
        }
        GraphProposal::Relationship {
            source_entity_id,
            target_entity_id,
            relationship_type,
        } => (
            "RELATIONSHIP",
            format!("{source_entity_id}:{relationship_type}:{target_entity_id}"),
            Some(source_entity_id.as_uuid()),
            Some(target_entity_id.as_uuid()),
            None,
        ),
        GraphProposal::Claim {
            subject_entity_id,
            predicate,
            ..
        } => (
            "CLAIM",
            format!("{subject_entity_id}:{predicate}"),
            Some(subject_entity_id.as_uuid()),
            None,
            None,
        ),
    };
    let id = existing_entity.unwrap_or_else(|| AssertionId::generate().as_uuid());
    sqlx::query("INSERT INTO institutional_records (id,tenant_id,project_id,record_type,identity_key,subject_id,target_id)
        VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (tenant_id,project_id,record_type,identity_key) DO NOTHING")
        .bind(id).bind(c.tenant_id.as_uuid()).bind(c.project_id.as_uuid()).bind(record_type).bind(&key).bind(subject).bind(target).execute(&mut **tx).await?;
    let (record,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM institutional_records
        WHERE tenant_id=$1 AND project_id=$2 AND record_type=$3 AND identity_key=$4 FOR UPDATE",
    )
    .bind(c.tenant_id.as_uuid())
    .bind(c.project_id.as_uuid())
    .bind(record_type)
    .bind(key)
    .fetch_one(&mut **tx)
    .await?;
    let lifecycle:Option<(bool,String)>=sqlx::query_as("SELECT
        (SELECT max(ownstate_classification_rank(h.security_classification))>ownstate_classification_rank($2)
            FROM institutional_versions h WHERE h.record_id=$1 AND h.status<>'CONFLICT'),v.status
        FROM institutional_versions v WHERE v.record_id=$1 AND v.status<>'CONFLICT'
        ORDER BY v.version_number DESC LIMIT 1")
        .bind(record).bind(classification.as_str()).fetch_optional(&mut **tx).await?;
    if let Some((above_ceiling, status)) = lifecycle
        && (above_ceiling || status == "REVOKED" || status == "QUARANTINED")
    {
        return Err(StorageError::Conflict(
            "institutional identity requires authorized lifecycle review".into(),
        ));
    }
    let previous_data: Option<(Value,)> = sqlx::query_as(
        "SELECT data FROM institutional_version_data WHERE record_id=$1 AND status IN ('ACTIVE','STALE')
        ORDER BY (status='ACTIVE') DESC,version_number DESC LIMIT 1",
    )
    .bind(record)
    .fetch_optional(&mut **tx)
    .await?;
    let previous: Option<InstitutionalVersion> = previous_data
        .map(|(data,)| serde_json::from_value(data).map_err(json_error))
        .transpose()?;
    if previous
        .as_ref()
        .is_some_and(|v| v.security_classification > classification)
    {
        return Err(StorageError::Conflict(
            "current identity is unavailable at candidate classification".into(),
        ));
    }
    if let Some(old) = &previous
        && old.proposal == c.proposal
    {
        sqlx::query("UPDATE institutional_candidates SET status='PROMOTED',result_version_id=$2 WHERE id=$1 AND status='PENDING'")
            .bind(c.id.as_uuid()).bind(old.id.as_uuid()).execute(&mut **tx).await?;
        return Ok(old.clone());
    }
    let (now,): (DateTime<Utc>,) = sqlx::query_as("SELECT clock_timestamp()")
        .fetch_one(&mut **tx)
        .await?;
    let boundary = previous.as_ref().map_or(now, |v| now.max(v.valid_from));
    let (max_version,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(max(version_number),0) FROM institutional_versions WHERE record_id=$1",
    )
    .bind(record)
    .fetch_one(&mut **tx)
    .await?;
    let content = serde_json::to_value(&c.proposal).map_err(json_error)?;
    // Lower-authority contradictory claims remain visible conflicts; they
    // cannot erase a human decision. Full type-dependent authority follows.
    let conflict = previous.as_ref().is_some_and(|old| {
        old.proposal != c.proposal
            && old.trust_level == TrustLevel::HumanExplicit
            && trust != TrustLevel::HumanExplicit
    });
    if !conflict && let Some(old) = &previous {
        sqlx::query(
            "UPDATE institutional_versions SET status='SUPERSEDED',valid_until=COALESCE(LEAST(valid_until,$2),$2) WHERE id=$1",
        )
        .bind(old.id.as_uuid())
        .bind(boundary)
        .execute(&mut **tx)
        .await?;
    }
    let version = InstitutionalVersion {
        id: AssertionVersionId::generate(),
        entity_id: (record_type == "ENTITY").then(|| record.into()),
        assertion_id: (record_type != "ENTITY").then(|| record.into()),
        tenant_id: c.tenant_id,
        project_id: c.project_id,
        version_number: max_version + 1,
        proposal: c.proposal.clone(),
        valid_from: boundary,
        valid_until: None,
        observed_at: c.observed_at,
        recorded_at: now,
        status: if conflict {
            KnowledgeStatus::Conflict
        } else {
            KnowledgeStatus::Active
        },
        trust_level: trust,
        confidence: c.confidence,
        security_classification: classification,
        evidence_event_ids: c.evidence_event_ids.clone(),
        candidate_id: c.id,
        supersedes_version_id: if conflict {
            None
        } else {
            previous.map(|v| v.id)
        },
    };
    sqlx::query("INSERT INTO institutional_versions (id,tenant_id,project_id,record_id,version_number,proposal,status,trust_level,
        security_classification,evidence_event_ids,candidate_id,valid_from,observed_at,recorded_at,supersedes_version_id,confidence)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)")
        .bind(version.id.as_uuid()).bind(c.tenant_id.as_uuid()).bind(c.project_id.as_uuid()).bind(record).bind(version.version_number)
        .bind(content).bind(version.status.as_str()).bind(trust.as_str()).bind(classification.as_str())
        .bind(c.evidence_event_ids.iter().map(EventId::as_uuid).collect::<Vec<_>>()).bind(c.id.as_uuid())
        .bind(boundary).bind(c.observed_at).bind(now).bind(version.supersedes_version_id.map(|v|v.as_uuid())).bind(c.confidence).execute(&mut **tx).await?;
    if record_type == "ENTITY" && !conflict {
        let ids = match &c.proposal {
            GraphProposal::Entity { external_ids, .. } => {
                serde_json::to_value(external_ids).map_err(json_error)?
            }
            _ => Value::Null,
        };
        sqlx::query("INSERT INTO institutional_identifiers (tenant_id,project_id,namespace,external_id,entity_id)
            SELECT $1,$2,x.key,x.value,$3 FROM jsonb_each_text($4::jsonb) x
            ON CONFLICT (tenant_id,project_id,namespace,external_id) DO NOTHING")
            .bind(c.tenant_id.as_uuid()).bind(c.project_id.as_uuid()).bind(record).bind(ids).execute(&mut **tx).await?;
    }
    sqlx::query(
        "UPDATE institutional_candidates SET status='PROMOTED',result_version_id=$2 WHERE id=$1 AND status='PENDING'",
    )
    .bind(c.id.as_uuid())
    .bind(version.id.as_uuid())
    .execute(&mut **tx)
    .await?;
    Ok(version)
}
