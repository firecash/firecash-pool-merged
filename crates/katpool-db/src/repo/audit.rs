//! Audit log aggregate — append-only trail of operator and
//! automated actions.
//!
//! By convention this table is **insert-only**. Application code
//! must never `UPDATE` or `DELETE` rows; future hardening will
//! enforce this with a per-role `GRANT` if any code path attempts it.

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::DbError;
use crate::repo::AuditLogId;

/// One audit-log row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditLogEntry {
    /// Synthetic primary key.
    pub id: AuditLogId,
    /// Wall-clock time of the action.
    pub occurred_at: DateTime<Utc>,
    /// Which subsystem or operator performed the action.
    pub actor: String,
    /// Free-form action identifier (`payout.broadcast`, `cycle.cancel`,
    /// `treasury.rotate`, …). Conventionally dot-separated noun.verb.
    pub action: String,
    /// Optional subject classification (`payout`, `payout_cycle`,
    /// `block`, …).
    pub subject_type: Option<String>,
    /// Optional subject id, paired with `subject_type` for indexed
    /// per-subject queries.
    pub subject_id: Option<i64>,
    /// Optional correlation id propagated from a `PoolEvent`.
    pub correlation_id: Option<Uuid>,
    /// JSONB blob carrying action-specific detail.
    pub payload: JsonValue,
}

/// Builder for [`append`] arguments. Keeps the call sites clean when
/// only a couple of optional fields are present.
#[derive(Debug, Clone)]
pub struct NewEntry<'a> {
    /// Subsystem or operator performing the action.
    pub actor: &'a str,
    /// Action identifier.
    pub action: &'a str,
    /// Optional subject type.
    pub subject_type: Option<&'a str>,
    /// Optional subject id.
    pub subject_id: Option<i64>,
    /// Optional correlation id.
    pub correlation_id: Option<Uuid>,
    /// Action-specific JSON blob; defaults to `{}` if you don't pass
    /// one in.
    pub payload: JsonValue,
}

impl<'a> NewEntry<'a> {
    /// Construct a minimal new entry (actor + action only).
    #[must_use]
    pub fn new(actor: &'a str, action: &'a str) -> Self {
        Self {
            actor,
            action,
            subject_type: None,
            subject_id: None,
            correlation_id: None,
            payload: JsonValue::Object(serde_json::Map::new()),
        }
    }

    /// Set the subject pair.
    #[must_use]
    pub const fn subject(mut self, kind: &'a str, id: i64) -> Self {
        self.subject_type = Some(kind);
        self.subject_id = Some(id);
        self
    }

    /// Attach a correlation id.
    #[must_use]
    pub const fn correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Attach the JSONB payload.
    #[must_use]
    pub fn payload(mut self, value: JsonValue) -> Self {
        self.payload = value;
        self
    }
}

/// Append one audit-log row. Always succeeds (modulo connection
/// errors) — the table has no CHECK constraints because the
/// `subject_type` taxonomy is intentionally open.
pub async fn append<'e, E>(executor: E, entry: NewEntry<'_>) -> Result<AuditLogId, DbError>
where
    E: PgExecutor<'e>,
{
    let id: AuditLogId = sqlx::query_scalar::<_, AuditLogId>(
        "
        INSERT INTO audit_log
            (actor, action, subject_type, subject_id, correlation_id, payload)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        ",
    )
    .bind(entry.actor)
    .bind(entry.action)
    .bind(entry.subject_type)
    .bind(entry.subject_id)
    .bind(entry.correlation_id)
    .bind(entry.payload)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// List entries for a given subject (`subject_type`, `subject_id`),
/// oldest-first. Powers the "show me everything that happened to
/// payout #N" operator query.
pub async fn list_for_subject<'e, E: PgExecutor<'e>>(
    executor: E,
    subject_type: &str,
    subject_id: i64,
    limit: i64,
) -> Result<Vec<AuditLogEntry>, DbError> {
    sqlx::query_as::<_, AuditLogEntry>(
        "SELECT id, occurred_at, actor, action, subject_type, subject_id, correlation_id, payload
           FROM audit_log
          WHERE subject_type = $1
            AND subject_id   = $2
          ORDER BY occurred_at ASC
          LIMIT $3",
    )
    .bind(subject_type)
    .bind(subject_id)
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(DbError::from)
}
