//! Typed, parameterized query execution with authorization before aggregation
//! and ranking. Semantic ranked IDs are revalidated before content is returned.
use crate::{Result, StorageError};
use chrono::{DateTime, Utc};
use ownstate_domain::*;
use pgvector::Vector;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct QueryRead<'a> {
    pub tenant: TenantId,
    pub constraints: &'a QueryConstraints,
    pub allowed: &'a [&'a str],
    pub as_of: DateTime<Utc>,
}
fn decode(error: serde_json::Error) -> StorageError {
    StorageError::Sqlx(sqlx::Error::Decode(Box::new(error)))
}
fn validate(q: QueryRead<'_>) -> Result<()> {
    q.constraints.validate().map_err(StorageError::Conflict)
}
fn entity(data: Value) -> Result<Entity> {
    let version: InstitutionalVersion = serde_json::from_value(data).map_err(decode)?;
    let id = version
        .entity_id
        .ok_or_else(|| StorageError::Conflict("expected entity version".into()))?;
    Ok(Entity { id, version })
}
fn kind(q: QueryRead<'_>) -> Result<Option<String>> {
    q.constraints
        .entity_kind
        .map(|k| {
            serde_json::to_value(k).map_err(decode).and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| StorageError::Conflict("invalid entity kind".into()))
            })
        })
        .transpose()
}
fn uuids(ids: &[EntityId]) -> Vec<Uuid> {
    ids.iter().map(EntityId::as_uuid).collect()
}
macro_rules! bind_entities {
    ($query:expr,$q:expr) => {{
        let q = $q;
        $query
            .bind(q.tenant.as_uuid())
            .bind(q.constraints.project_id.as_uuid())
            .bind(q.allowed)
            .bind(q.as_of)
            .bind(kind(q)?)
            .bind(uuids(&q.constraints.entity_ids))
            .bind(
                q.constraints
                    .relationship
                    .as_ref()
                    .map(|r| r.relationship_type.as_str()),
            )
            .bind(
                q.constraints
                    .relationship
                    .as_ref()
                    .map_or_else(Vec::new, |r| uuids(&r.target_entity_ids)),
            )
            .bind(
                q.constraints
                    .trust_levels
                    .iter()
                    .map(TrustLevel::as_str)
                    .collect::<Vec<_>>(),
            )
            .bind(&q.constraints.source_types)
    }};
}
pub async fn structured<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    operation: StructuredOperation,
) -> Result<StructuredResult> {
    validate(q)?;
    match operation {
        StructuredOperation::Entities => {
            let rows:Vec<(Value,)>=bind_entities!(sqlx::query_as("SELECT data FROM ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ORDER BY record_id LIMIT $11"),q).bind(i64::from(q.constraints.limit)).fetch_all(exec).await?;
            Ok(StructuredResult::Entities {
                entities: rows
                    .into_iter()
                    .map(|(d,)| entity(d))
                    .collect::<Result<_>>()?,
            })
        }
        StructuredOperation::Count => {
            let (count, ids):(i64,Vec<Uuid>)=bind_entities!(sqlx::query_as("WITH e AS (SELECT * FROM ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)) SELECT count(DISTINCT record_id),ARRAY(SELECT DISTINCT eid FROM e CROSS JOIN LATERAL unnest(evidence_event_ids)eid ORDER BY eid) FROM e"),q).fetch_one(exec).await?;
            Ok(StructuredResult::Count {
                count,
                evidence_event_ids: ids.into_iter().map(Into::into).collect(),
            })
        }
        StructuredOperation::GroupByJurisdiction => {
            let rows:Vec<(Option<Value>,i64,Vec<Uuid>)>=bind_entities!(sqlx::query_as("WITH entities AS (SELECT * FROM ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)),
                membership AS (SELECT e.record_id,e.evidence_event_ids,edge.evidence_event_ids AS edge_evidence,j.record_id AS jurisdiction_id,j.data AS jurisdiction,j.evidence_event_ids AS jurisdiction_evidence
                    FROM entities e LEFT JOIN LATERAL(SELECT r.* FROM ownstate_query_institutional($1,$2,$3,$4)r
                        JOIN ownstate_query_institutional($1,$2,$3,$4)t ON t.record_id=r.target_id AND t.record_type='ENTITY' AND t.proposal->>'kind'='JURISDICTION'
                        WHERE r.record_type='RELATIONSHIP' AND r.subject_id=e.record_id AND lower(r.proposal->>'relationship_type') IN('registered_in','incorporated_in','jurisdiction'))edge ON true
                    LEFT JOIN ownstate_query_institutional($1,$2,$3,$4)j ON j.record_id=edge.target_id AND j.record_type='ENTITY'),
                counts AS (SELECT jurisdiction_id,count(DISTINCT record_id) AS count FROM membership GROUP BY jurisdiction_id)
                SELECT (SELECT m.jurisdiction FROM membership m WHERE m.jurisdiction_id IS NOT DISTINCT FROM c.jurisdiction_id LIMIT 1),c.count,
                    ARRAY(SELECT DISTINCT eid FROM membership m CROSS JOIN LATERAL unnest(m.evidence_event_ids||COALESCE(m.edge_evidence,'{}'::uuid[])||COALESCE(m.jurisdiction_evidence,'{}'::uuid[]))eid
                        WHERE m.jurisdiction_id IS NOT DISTINCT FROM c.jurisdiction_id ORDER BY eid)
                FROM counts c ORDER BY c.jurisdiction_id NULLS LAST"),q).fetch_all(exec).await?;
            Ok(StructuredResult::GroupByJurisdiction {
                groups: rows
                    .into_iter()
                    .map(|(d, count, ids)| {
                        Ok(JurisdictionCount {
                            jurisdiction: d.map(entity).transpose()?,
                            count,
                            evidence_event_ids: ids.into_iter().map(Into::into).collect(),
                        })
                    })
                    .collect::<Result<_>>()?,
            })
        }
    }
}
pub async fn hybrid_entities<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
) -> Result<Vec<EntityId>> {
    validate(q)?;
    // Output IDs are bounded. Hybrid SQL legs independently apply the entire
    // entity relation before their ranking limits.
    let rows:Vec<(Uuid,)>=bind_entities!(sqlx::query_as("SELECT DISTINCT record_id FROM ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ORDER BY record_id LIMIT $11"),q).bind(i64::from(q.constraints.limit)).fetch_all(exec).await?;
    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
pub async fn relationships<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    id: EntityId,
    direction: RelationshipDirection,
    relationship_type: Option<&str>,
) -> Result<Vec<InstitutionalVersion>> {
    validate(q)?;
    let direction = match direction {
        RelationshipDirection::Outgoing => "OUTGOING",
        RelationshipDirection::Incoming => "INCOMING",
        RelationshipDirection::Both => "BOTH",
    };
    let rows:Vec<(Value,)>=bind_entities!(sqlx::query_as("SELECT r.data FROM ownstate_query_institutional($1,$2,$3,$4)r
        WHERE r.record_type='RELATIONSHIP' AND ($12::text IS NULL OR r.proposal->>'relationship_type'=$12)
        AND (($13 IN('OUTGOING','BOTH') AND r.subject_id=$11) OR ($13 IN('INCOMING','BOTH') AND r.target_id=$11))
        AND (cardinality($9::text[])=0 OR r.trust_level=ANY($9))
        AND (cardinality($10::text[])=0 OR EXISTS(SELECT 1 FROM interaction_events ev WHERE ev.id=ANY(r.evidence_event_ids) AND ev.source=ANY($10)))
        AND EXISTS(SELECT 1 FROM ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,'{}'::text[],'{}'::text[])e
            WHERE e.record_id=CASE WHEN r.subject_id=$11 THEN r.target_id ELSE r.subject_id END)
        ORDER BY r.recorded_at DESC,r.id LIMIT $14"),q).bind(id.as_uuid()).bind(relationship_type).bind(direction).bind(i64::from(q.constraints.limit)).fetch_all(exec).await?;
    rows.into_iter()
        .map(|(d,)| serde_json::from_value(d).map_err(decode))
        .collect()
}
macro_rules! bind_knowledge {
    ($query:expr,$q:expr,$entities:expr) => {{
        let q = $q;
        $query
            .bind(q.tenant.as_uuid())
            .bind(q.constraints.project_id.as_uuid())
            .bind(q.allowed)
            .bind(q.as_of)
            .bind(
                q.constraints
                    .knowledge_kinds
                    .iter()
                    .map(KnowledgeKind::as_str)
                    .collect::<Vec<_>>(),
            )
            .bind(
                $entities
                    .or_else(|| {
                        if q.constraints.entity_ids.is_empty() {
                            None
                        } else {
                            Some(q.constraints.entity_ids.as_slice())
                        }
                    })
                    .map(uuids),
            )
            .bind(
                q.constraints
                    .trust_levels
                    .iter()
                    .map(TrustLevel::as_str)
                    .collect::<Vec<_>>(),
            )
            .bind(&q.constraints.source_types)
            .bind(kind(q)?)
            .bind(
                q.constraints
                    .relationship
                    .as_ref()
                    .map(|r| r.relationship_type.as_str()),
            )
            .bind(
                q.constraints
                    .relationship
                    .as_ref()
                    .map_or_else(Vec::new, |r| uuids(&r.target_entity_ids)),
            )
    }};
}
pub async fn semantic_fts<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    query: &str,
    matched_entities: Option<&[EntityId]>,
) -> Result<Vec<KnowledgeVersionId>> {
    validate(q)?;
    let rows:Vec<(Uuid,)>=bind_knowledge!(sqlx::query_as("SELECT id FROM ownstate_query_knowledge($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)kv
        WHERE content_tsv @@ websearch_to_tsquery('english',$12)
        ORDER BY ts_rank(content_tsv,websearch_to_tsquery('english',$12)) DESC,created_at DESC,id LIMIT $13"),q,matched_entities)
        .bind(query).bind(i64::from(q.constraints.limit)*4).fetch_all(exec).await?;
    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
pub async fn semantic_vector<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    embedding: Vector,
    model: &str,
    matched_entities: Option<&[EntityId]>,
) -> Result<Vec<KnowledgeVersionId>> {
    validate(q)?;
    let rows: Vec<(Uuid,)> = bind_knowledge!(
        sqlx::query_as(
            "SELECT kv.id FROM ownstate_query_knowledge($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)kv
        JOIN embeddings e ON e.entity_id=kv.id AND e.entity_type='KNOWLEDGE_VERSION' AND e.model=$12
        ORDER BY e.embedding <=> $13,kv.created_at DESC,kv.id LIMIT $14"
        ),
        q,
        matched_entities
    )
    .bind(model)
    .bind(embedding)
    .bind(i64::from(q.constraints.limit) * 4)
    .fetch_all(exec)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
pub async fn load_ranked<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    ids: &[KnowledgeVersionId],
) -> Result<Vec<(KnowledgeVersion, KnowledgeItem)>> {
    validate(q)?;
    if ids.len() > 800 {
        return Err(StorageError::Conflict(
            "ranked candidate IDs exceed 800".into(),
        ));
    }
    let matches = if q.constraints.entity_ids.is_empty() {
        None
    } else {
        Some(q.constraints.entity_ids.as_slice())
    };
    let rows:Vec<(Value,Value)>=bind_knowledge!(sqlx::query_as("SELECT to_jsonb(kv)-'content_tsv',to_jsonb(ki)
        FROM ownstate_query_knowledge($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)kv JOIN knowledge_items ki ON ki.id=kv.knowledge_item_id
        WHERE kv.id=ANY($12) ORDER BY array_position($12,kv.id)"),q,matches)
        .bind(ids.iter().map(KnowledgeVersionId::as_uuid).collect::<Vec<_>>()).fetch_all(exec).await?;
    rows.into_iter()
        .map(|(v, i)| {
            Ok((
                serde_json::from_value(v).map_err(decode)?,
                serde_json::from_value(i).map_err(decode)?,
            ))
        })
        .collect()
}
pub async fn insert_knowledge_links(
    tx: &mut Transaction<'_, Postgres>,
    tenant: TenantId,
    project: ProjectId,
    version: KnowledgeVersionId,
    ids: &[EntityId],
) -> Result<()> {
    for id in ids {
        let result=sqlx::query("INSERT INTO knowledge_entity_links(tenant_id,project_id,knowledge_version_id,entity_id,entity_version_id,evidence_event_ids)
            SELECT $1,$2,$3,$4,e.id,e.evidence_event_ids FROM institutional_version_data e
            WHERE e.tenant_id=$1 AND e.project_id=$2 AND e.record_id=$4 AND e.record_type='ENTITY' AND e.status='ACTIVE'
            AND e.valid_from<=statement_timestamp() AND (e.valid_until IS NULL OR e.valid_until>statement_timestamp())
            ON CONFLICT(knowledge_version_id,entity_id) DO NOTHING")
            .bind(tenant.as_uuid()).bind(project.as_uuid()).bind(version.as_uuid()).bind(id.as_uuid()).execute(&mut **tx).await?;
        if result.rows_affected() == 0 {
            let (exists,):(bool,)=sqlx::query_as("SELECT EXISTS(SELECT 1 FROM knowledge_entity_links WHERE tenant_id=$1 AND project_id=$2 AND knowledge_version_id=$3 AND entity_id=$4)")
                .bind(tenant.as_uuid()).bind(project.as_uuid()).bind(version.as_uuid()).bind(id.as_uuid()).fetch_one(&mut **tx).await?;
            if !exists {
                return Err(StorageError::Conflict(
                    "linked entity is unavailable".into(),
                ));
            }
        }
    }
    Ok(())
}

pub struct TemporalSemanticInput<'a> {
    pub embedding: Vector,
    pub model: &'a str,
}

pub async fn temporal_changes<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    subject: &TemporalSubject,
) -> Result<TemporalChanges> {
    temporal_changes_with_semantic(exec, q, start, end, subject, None).await
}

pub async fn temporal_changes_with_semantic<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    subject: &TemporalSubject,
    semantic_input: Option<TemporalSemanticInput<'_>>,
) -> Result<TemporalChanges> {
    validate(q)?;
    if start >= end || end - start > chrono::Duration::days(366) {
        return Err(StorageError::Conflict("invalid temporal interval".into()));
    }
    let semantic = matches!(subject, TemporalSubject::Semantic { .. });
    let text = match subject {
        TemporalSubject::Semantic { query } => Some(query.as_str()),
        _ => None,
    };
    let (embedding, model) = semantic_input.map_or((None, None), |input| {
        (Some(input.embedding), Some(input.model))
    });
    let rows: Vec<(String, Value, Option<Value>, i64)> = bind_entities!(sqlx::query_as("WITH population AS (
        SELECT 'INSTITUTIONAL'::text AS type,v.data AS data,NULL::jsonb AS item,v.recorded_at AS ordering,v.id,NULL::tsvector AS lexical
        FROM institutional_version_data v
        WHERE NOT $13 AND $4::timestamptz IS NOT NULL AND v.tenant_id=$1 AND v.project_id=$2
        AND (v.recorded_at>$11 AND v.recorded_at<=$12 OR v.valid_from>$11 AND v.valid_from<=$12 OR v.valid_until>$11 AND v.valid_until<=$12)
        AND EXISTS(SELECT 1 FROM ownstate_query_institutional($1,$2,$3,v.valid_from)visible WHERE visible.id=v.id)
        AND (cardinality($9::text[])=0 OR v.trust_level=ANY($9))
        AND (cardinality($10::text[])=0 OR EXISTS(SELECT 1 FROM interaction_events ev WHERE ev.id=ANY(v.evidence_event_ids) AND ev.source=ANY($10)))
        AND EXISTS(SELECT 1 FROM ownstate_query_entities($1,$2,$3,v.valid_from,$5,$6,$7,$8,'{}'::text[],'{}'::text[])e
            WHERE e.record_id=v.record_id OR e.record_id=v.subject_id OR e.record_id=v.target_id)
        UNION ALL
        SELECT 'KNOWLEDGE',to_jsonb(kv)-'content_tsv',to_jsonb(ki),kv.created_at,kv.id,kv.content_tsv
        FROM knowledge_versions_with_safe_evidence kv JOIN knowledge_items ki ON ki.id=kv.knowledge_item_id
        WHERE $13 AND ki.tenant_id=$1 AND ki.project_id=$2
        AND (kv.created_at>$11 AND kv.created_at<=$12 OR kv.valid_from>$11 AND kv.valid_from<=$12 OR kv.valid_until>$11 AND kv.valid_until<=$12)
        AND EXISTS(SELECT 1 FROM ownstate_query_knowledge($1,$2,$3,kv.valid_from,$15,
            CASE WHEN cardinality($6)=0 THEN NULL::uuid[] ELSE $6 END,$9,$10,$5,$7,$8)visible WHERE visible.id=kv.id)
    ), lexical_leg AS (
        SELECT id,row_number() OVER(ORDER BY ts_rank(lexical,websearch_to_tsquery('english',$14)) DESC,ordering DESC,id) AS rank
        FROM population WHERE type='KNOWLEDGE' AND lexical @@ websearch_to_tsquery('english',$14)
        ORDER BY rank LIMIT $17
    ), dense_leg AS (
        SELECT p.id,row_number() OVER(ORDER BY e.embedding <=> $18,ordering DESC,p.id) AS rank
        FROM population p JOIN embeddings e ON e.entity_id=p.id AND e.entity_type='KNOWLEDGE_VERSION' AND e.model=$19
        WHERE p.type='KNOWLEDGE' AND $18::vector IS NOT NULL ORDER BY rank LIMIT $17
    ), fused AS (
        SELECT id,sum(1.0/(60+rank)) AS score FROM (SELECT * FROM lexical_leg UNION ALL SELECT * FROM dense_leg)legs GROUP BY id
    ), selected AS (
        SELECT p.type,p.data,p.item,p.ordering,p.id,COALESCE(f.score,0)*COALESCE((SELECT weight FROM unnest($20::text[],$21::float8[])weights(level,weight) WHERE level=p.data->>'trust_level'),1) AS score FROM population p LEFT JOIN fused f ON f.id=p.id
        WHERE p.type='INSTITUTIONAL' OR f.id IS NOT NULL
    ), bounded AS (
        SELECT type,data,item,count(*) OVER()total FROM selected ORDER BY score DESC,CASE WHEN type='INSTITUTIONAL' THEN ordering END,id LIMIT $16
    ) SELECT type,data,item,total FROM bounded"),q)
        .bind(start).bind(end).bind(semantic).bind(text)
        .bind(q.constraints.knowledge_kinds.iter().map(KnowledgeKind::as_str).collect::<Vec<_>>())
        .bind(i64::from(q.constraints.limit)).bind(i64::from(q.constraints.limit)*4)
        .bind(embedding).bind(model)
        .bind(TrustLevel::ALL.iter().map(TrustLevel::as_str).collect::<Vec<_>>())
        .bind(TrustLevel::ALL.iter().map(TrustLevel::rank_weight).collect::<Vec<_>>())
        .fetch_all(exec).await?;
    let total = rows.first().map_or(0, |r| r.3);
    let mut result = TemporalChanges {
        institutional: Vec::new(),
        knowledge: Vec::new(),
        total,
        has_more: total > i64::from(q.constraints.limit),
        candidate_limit: semantic.then_some(q.constraints.limit * 4),
    };
    for (kind, data, item, _) in rows {
        if kind == "INSTITUTIONAL" {
            result
                .institutional
                .push(serde_json::from_value(data).map_err(decode)?);
        } else {
            result.knowledge.push((
                serde_json::from_value(data).map_err(decode)?,
                serde_json::from_value(
                    item.ok_or_else(|| StorageError::Conflict("missing knowledge item".into()))?,
                )
                .map_err(decode)?,
            ));
        }
    }
    Ok(result)
}

pub async fn snapshot_timestamp<'e>(exec: impl sqlx::PgExecutor<'e>) -> Result<DateTime<Utc>> {
    let (at,): (DateTime<Utc>,) = sqlx::query_as("SELECT clock_timestamp()")
        .fetch_one(exec)
        .await?;
    Ok(at)
}

pub async fn hybrid_fts<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    query: &str,
) -> Result<Vec<KnowledgeVersionId>> {
    validate(q)?;
    let rows:Vec<(Uuid,)>=bind_entities!(sqlx::query_as("SELECT kv.id
        FROM ownstate_query_knowledge($1,$2,$3,$4,$11,CASE WHEN cardinality($6)=0 THEN NULL::uuid[] ELSE $6 END,$9,$10,$5,$7,$8)kv
        WHERE kv.content_tsv @@ websearch_to_tsquery('english',$12)
        AND EXISTS(SELECT 1 FROM knowledge_entity_links l JOIN ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)e ON e.record_id=l.entity_id
            WHERE l.knowledge_version_id=kv.id AND l.tenant_id=$1 AND l.project_id=$2)
        ORDER BY ts_rank(kv.content_tsv,websearch_to_tsquery('english',$12)) DESC,kv.created_at DESC,kv.id LIMIT $13"),q)
        .bind(q.constraints.knowledge_kinds.iter().map(KnowledgeKind::as_str).collect::<Vec<_>>())
        .bind(query).bind(i64::from(q.constraints.limit)*4).fetch_all(exec).await?;
    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
pub async fn hybrid_vector<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    embedding: Vector,
    model: &str,
) -> Result<Vec<KnowledgeVersionId>> {
    validate(q)?;
    let rows:Vec<(Uuid,)>=bind_entities!(sqlx::query_as("SELECT kv.id
        FROM ownstate_query_knowledge($1,$2,$3,$4,$11,CASE WHEN cardinality($6)=0 THEN NULL::uuid[] ELSE $6 END,$9,$10,$5,$7,$8)kv
        JOIN embeddings emb ON emb.entity_id=kv.id AND emb.entity_type='KNOWLEDGE_VERSION' AND emb.model=$12
        WHERE EXISTS(SELECT 1 FROM knowledge_entity_links l JOIN ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)e ON e.record_id=l.entity_id
            WHERE l.knowledge_version_id=kv.id AND l.tenant_id=$1 AND l.project_id=$2)
        ORDER BY emb.embedding <=> $13,kv.created_at DESC,kv.id LIMIT $14"),q)
        .bind(q.constraints.knowledge_kinds.iter().map(KnowledgeKind::as_str).collect::<Vec<_>>())
        .bind(model).bind(embedding).bind(i64::from(q.constraints.limit)*4).fetch_all(exec).await?;
    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
pub async fn load_hybrid_ranked<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    q: QueryRead<'_>,
    ids: &[KnowledgeVersionId],
) -> Result<Vec<(KnowledgeVersion, KnowledgeItem)>> {
    validate(q)?;
    if ids.len() > 800 {
        return Err(StorageError::Conflict(
            "ranked candidate IDs exceed 800".into(),
        ));
    }
    let rows:Vec<(Value,Value)>=bind_entities!(sqlx::query_as("SELECT to_jsonb(kv)-'content_tsv',to_jsonb(ki)
        FROM ownstate_query_knowledge($1,$2,$3,$4,$11,CASE WHEN cardinality($6)=0 THEN NULL::uuid[] ELSE $6 END,$9,$10,$5,$7,$8)kv JOIN knowledge_items ki ON ki.id=kv.knowledge_item_id
        WHERE kv.id=ANY($12)
        AND EXISTS(SELECT 1 FROM knowledge_entity_links l JOIN ownstate_query_entities($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)e ON e.record_id=l.entity_id
            WHERE l.knowledge_version_id=kv.id AND l.tenant_id=$1 AND l.project_id=$2)
        ORDER BY array_position($12,kv.id)"),q)
        .bind(q.constraints.knowledge_kinds.iter().map(KnowledgeKind::as_str).collect::<Vec<_>>())
        .bind(ids.iter().map(KnowledgeVersionId::as_uuid).collect::<Vec<_>>()).fetch_all(exec).await?;
    rows.into_iter()
        .map(|(v, i)| {
            Ok((
                serde_json::from_value(v).map_err(decode)?,
                serde_json::from_value(i).map_err(decode)?,
            ))
        })
        .collect()
}
