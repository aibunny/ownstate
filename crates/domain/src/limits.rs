//! Input size limits enforced by the application layer. Large immutable
//! content belongs in object storage (future milestone), not in these rows.

/// Maximum bytes of `content` accepted for a single interaction event or
/// knowledge version.
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;

/// Maximum length of a knowledge subject key.
pub const MAX_SUBJECT_KEY_LEN: usize = 200;

/// Maximum length of project names.
pub const MAX_NAME_LEN: usize = 200;

/// Maximum length of a context compilation task description.
pub const MAX_TASK_LEN: usize = 8 * 1024;

/// Maximum number of events accepted in one append batch.
pub const MAX_EVENTS_PER_BATCH: usize = 500;

/// Maximum number of evidence references on one candidate.
pub const MAX_EVIDENCE_REFS: usize = 50;

/// Maximum `max_items` for context compilation and search.
pub const MAX_CONTEXT_ITEMS: usize = 100;

/// How far ahead of the next automatic sequence an explicit event sequence
/// may jump. Prevents a single crafted event (e.g. sequence = i64::MAX) from
/// exhausting a session's sequence space and poisoning future appends.
pub const MAX_SEQUENCE_GAP: i64 = 1_000_000;
