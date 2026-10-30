//! Stream-driven Salesforce CDC capture.

use std::{pin::Pin, time::Duration};

use async_trait::async_trait;
use force::auth::Authenticator;
use force_pubsub::{PubSubError, PubSubEvent, PubSubHandler, ReplayId, ReplayPreset};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::GenericClient;

use crate::{
    ObjectSync,
    error::ForceSyncError,
    identity::SyncKey,
    model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    store::pg::{AppendResult, DeadLetter, PgStore},
};

/// A stream of decoded Change Data Capture events.
pub type CdcEventStream =
    Pin<Box<dyn Stream<Item = Result<PubSubEvent<Value>, PubSubError>> + Send>>;

/// A source of Salesforce Change Data Capture events.
///
/// [`PubSubHandler`] implements it. A test or another transport can provide
/// its own.
#[async_trait]
pub trait CdcSource: Send + Sync {
    /// Subscribes to `topic`, starting at `replay`.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription cannot be opened.
    async fn subscribe(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<CdcEventStream, ForceSyncError>;
}

#[async_trait]
impl<A: Authenticator + 'static> CdcSource for PubSubHandler<A> {
    async fn subscribe(
        &self,
        topic: &str,
        replay: ReplayPreset,
    ) -> Result<CdcEventStream, ForceSyncError> {
        Ok(Self::subscribe(self, topic, replay).await?)
    }
}

/// Where a CDC stream starts when it has no checkpoint yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CdcStart {
    /// Only events published after the first subscription.
    #[default]
    Latest,
    /// Every event in the Salesforce retention window (72 hours).
    Earliest,
}

/// One Salesforce Change Data Capture subscription for one synced object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdcStream {
    name: String,
    topic: String,
    tenant: String,
    object_name: String,
    start: CdcStart,
}

impl CdcStream {
    /// A stream on the standard change event channel of `object_name`.
    ///
    /// The channel is `/data/{Object}ChangeEvent`, or
    /// `/data/{Object}__ChangeEvent` for a custom object (`Invoice__c`). The
    /// checkpoint name is `salesforce:{object_name}`.
    #[must_use]
    pub fn new(tenant: impl Into<String>, object_name: impl Into<String>) -> Self {
        let object_name = object_name.into();
        let channel = object_name
            .strip_suffix("__c")
            .map_or_else(|| object_name.clone(), |base| format!("{base}__"));
        Self {
            name: format!("salesforce:{object_name}"),
            topic: format!("/data/{channel}ChangeEvent"),
            tenant: tenant.into(),
            object_name,
            start: CdcStart::default(),
        }
    }

    /// Uses another channel, such as a custom channel.
    #[must_use]
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = topic.into();
        self
    }

    /// Uses another checkpoint name. It must be unique per stream.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Sets where the stream starts when it has no checkpoint.
    #[must_use]
    pub const fn starting_at(mut self, start: CdcStart) -> Self {
        self.start = start;
        self
    }

    /// The checkpoint name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The Pub/Sub channel.
    #[must_use]
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// The tenant of the captured records.
    #[must_use]
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// The synced object name.
    #[must_use]
    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    /// Where the stream starts when it has no checkpoint.
    #[must_use]
    pub const fn start(&self) -> CdcStart {
        self.start
    }
}

/// The bounds of one capture window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdcWindow {
    /// The longest time the window stays subscribed.
    pub max_duration: Duration,
    /// The most events the window reads.
    pub max_events: usize,
}

impl Default for CdcWindow {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_secs(50),
            max_events: 1000,
        }
    }
}

/// Why a capture window ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CdcWindowEnd {
    /// The window reached its `max_duration`.
    Deadline,
    /// The window read `max_events` events.
    MaxEvents,
    /// The source ended the stream.
    StreamEnded,
}

/// What one capture window did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdcWindowReport {
    /// Events written to the journal, each with an apply task.
    pub captured: usize,
    /// Events already in the journal, such as a redelivery after a restart.
    pub duplicates: usize,
    /// Events moved to `sync_dead_letter` because their content is invalid.
    pub quarantined: usize,
    /// Why the window ended.
    pub end: CdcWindowEnd,
}

/// What capturing one event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventOutcome {
    Captured,
    Duplicate,
    Quarantined,
}

/// Whether `error` comes from the content of one event.
///
/// Such an event can never be captured, so it is quarantined. A
/// configuration or database error fails the window instead.
fn event_content_error(error: &ForceSyncError) -> bool {
    matches!(
        error,
        ForceSyncError::InvalidStoredValue {
            field: "external_id",
            ..
        } | ForceSyncError::EmptySyncKeyPart { .. }
    )
}

fn change_operation(payload: &Value) -> ChangeOperation {
    match payload
        .get("ChangeEventHeader")
        .and_then(|header| header.get("changeType"))
        .and_then(Value::as_str)
    {
        Some("DELETE") => ChangeOperation::Delete,
        _ => ChangeOperation::Upsert,
    }
}

fn replay_id_to_position(replay_id: &ReplayId) -> Result<i64, ForceSyncError> {
    if replay_id.as_bytes().len() > 8 {
        return Err(ForceSyncError::InvalidStoredValue {
            field: "replay_id",
            value: format!("{} bytes", replay_id.as_bytes().len()),
        });
    }

    let mut padded = [0u8; 8];
    let start = 8 - replay_id.as_bytes().len();
    padded[start..].copy_from_slice(replay_id.as_bytes());

    let unsigned = u64::from_be_bytes(padded);
    i64::try_from(unsigned).map_err(|_| ForceSyncError::InvalidStoredValue {
        field: "replay_id",
        value: unsigned.to_string(),
    })
}

fn replay_id_from_position(position: i64) -> Result<ReplayId, ForceSyncError> {
    let unsigned = u64::try_from(position).map_err(|_| ForceSyncError::InvalidStoredValue {
        field: "cursor_position",
        value: position.to_string(),
    })?;
    let bytes = unsigned.to_be_bytes();
    let first_non_zero = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    Ok(ReplayId::from_bytes(bytes[first_non_zero..].to_vec()))
}

fn build_envelope(
    tenant: &str,
    object: &ObjectSync,
    payload: Value,
    replay_position: i64,
) -> Result<ChangeEnvelope, ForceSyncError> {
    let external_id_field =
        object
            .external_id_field()
            .ok_or(ForceSyncError::MissingConfiguration {
                field: "external_id_field",
            })?;
    let external_id = payload
        .get(external_id_field)
        .and_then(Value::as_str)
        .ok_or_else(|| ForceSyncError::InvalidStoredValue {
            field: "external_id",
            value: payload.to_string(),
        })?;

    Ok(ChangeEnvelope::new(
        SyncKey::new(
            tenant.to_owned(),
            object.object_name().to_owned(),
            external_id.to_owned(),
        )?,
        SourceSystem::Salesforce,
        change_operation(&payload),
        chrono::Utc::now(),
        payload,
    )
    .with_cursor(SourceCursor::SalesforceReplayId(replay_position)))
}

async fn capture_event_in_tx<C>(
    client: &C,
    stream_name: &str,
    tenant: &str,
    object: &ObjectSync,
    payload: Value,
    replay_id: &ReplayId,
) -> Result<bool, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let replay_position = replay_id_to_position(replay_id)?;
    let envelope = build_envelope(tenant, object, payload, replay_position)?;
    let inserted = journal_envelope_in_tx(client, &envelope).await?;
    advance_checkpoint_in_tx(client, stream_name, replay_position).await?;
    Ok(inserted)
}

async fn journal_envelope_in_tx<C>(
    client: &C,
    envelope: &ChangeEnvelope,
) -> Result<bool, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    Ok(
        match PgStore::append_journal_if_new_in_tx(client, envelope).await? {
            AppendResult::Inserted { journal_id } => {
                PgStore::enqueue_apply_task_in_tx(client, journal_id, 0).await?;
                true
            }
            AppendResult::Duplicate => false,
        },
    )
}

async fn advance_checkpoint_in_tx<C>(
    client: &C,
    stream_name: &str,
    replay_position: i64,
) -> Result<(), ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let cursor = SourceCursor::SalesforceReplayId(replay_position).as_db_value();
    PgStore::advance_checkpoint_if_greater_in_tx(client, stream_name, replay_position, &cursor)
        .await?;
    Ok(())
}

/// Captures one event, or quarantines it when its content is invalid.
///
/// The journal row, the apply task, any dead letter, and the checkpoint
/// commit together. A crash before the commit leaves the checkpoint behind
/// the event, so the next subscription delivers it again.
async fn capture_or_quarantine_in_tx<C>(
    client: &C,
    stream: &CdcStream,
    object: &ObjectSync,
    payload: Value,
    replay_id: &ReplayId,
) -> Result<EventOutcome, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let replay_position = replay_id_to_position(replay_id)?;
    let outcome = match build_envelope(stream.tenant(), object, payload.clone(), replay_position) {
        Ok(envelope) => {
            if journal_envelope_in_tx(client, &envelope).await? {
                EventOutcome::Captured
            } else {
                EventOutcome::Duplicate
            }
        }
        Err(error) if event_content_error(&error) => {
            let dead_letter = DeadLetter {
                task_id: None,
                tenant: Some(stream.tenant().to_owned()),
                object_name: Some(stream.object_name().to_owned()),
                external_id: None,
                error_message: format!(
                    "CDC event {} on {}: {error}",
                    SourceCursor::SalesforceReplayId(replay_position).as_db_value(),
                    stream.topic()
                ),
                payload: Some(payload),
            };
            crate::store::pg::dead_letter::insert_dead_letter_in_tx(client, &dead_letter).await?;
            EventOutcome::Quarantined
        }
        Err(error) => return Err(error),
    };
    advance_checkpoint_in_tx(client, stream.name(), replay_position).await?;
    Ok(outcome)
}

/// Captures one bounded window of a CDC stream.
///
/// The window subscribes from the stream checkpoint, or from
/// [`CdcStream::start`] when there is none. It ends after `window` elapses,
/// after `window.max_events` events, or when the source ends the stream. Each
/// event commits its journal row, apply task and checkpoint in one
/// transaction. An event whose content is invalid, such as one with no
/// external ID, goes to `sync_dead_letter` and the checkpoint moves past it.
///
/// Call it in a loop to follow a stream. The durable runtime does that in a
/// harvest workflow.
///
/// # Errors
///
/// Returns an error if the object has no external ID field, the
/// subscription or the stream fails, or a database write fails. The events
/// committed before the error stay committed.
pub async fn capture_cdc_window(
    store: &PgStore,
    source: &dyn CdcSource,
    stream: &CdcStream,
    object: &ObjectSync,
    window: CdcWindow,
) -> Result<CdcWindowReport, ForceSyncError> {
    object
        .external_id_field()
        .ok_or(ForceSyncError::MissingConfiguration {
            field: "external_id_field",
        })?;

    let replay = load_replay_id(store, stream.name()).await?.map_or_else(
        || match stream.start() {
            CdcStart::Latest => ReplayPreset::Latest,
            CdcStart::Earliest => ReplayPreset::Earliest,
        },
        ReplayPreset::Custom,
    );
    let mut events = source.subscribe(stream.topic(), replay).await?;
    let deadline = tokio::time::Instant::now() + window.max_duration;
    let mut report = CdcWindowReport {
        captured: 0,
        duplicates: 0,
        quarantined: 0,
        end: CdcWindowEnd::Deadline,
    };
    let mut read = 0usize;

    while read < window.max_events {
        let Ok(next) = tokio::time::timeout_at(deadline, events.next()).await else {
            report.end = CdcWindowEnd::Deadline;
            return Ok(report);
        };
        let Some(item) = next else {
            report.end = CdcWindowEnd::StreamEnded;
            return Ok(report);
        };
        let PubSubEvent::Event(message) = item? else {
            continue;
        };
        read += 1;
        let stream = stream.clone();
        let object = object.clone();
        let outcome = store
            .with_transaction(|tx| {
                Box::pin(async move {
                    capture_or_quarantine_in_tx(
                        tx,
                        &stream,
                        &object,
                        message.payload,
                        &message.replay_id,
                    )
                    .await
                })
            })
            .await?;
        match outcome {
            EventOutcome::Captured => report.captured += 1,
            EventOutcome::Duplicate => report.duplicates += 1,
            EventOutcome::Quarantined => report.quarantined += 1,
        }
    }

    report.end = CdcWindowEnd::MaxEvents;
    Ok(report)
}

/// Captures a decoded CDC stream into the sync journal and checkpoints replay positions.
///
/// # Errors
///
/// Returns an error if Pub/Sub decoding, journal writes, or checkpoint updates fail.
pub async fn capture_stream<S>(
    store: &PgStore,
    stream_name: &str,
    tenant: &str,
    object: &ObjectSync,
    mut stream: S,
) -> Result<usize, ForceSyncError>
where
    S: Stream<Item = Result<PubSubEvent<Value>, force_pubsub::PubSubError>> + Unpin,
{
    let mut captured = 0usize;

    while let Some(item) = stream.next().await {
        match item? {
            PubSubEvent::Event(message) => {
                let stream_name = stream_name.to_owned();
                let tenant = tenant.to_owned();
                let object = object.clone();
                if store
                    .with_transaction(|tx| {
                        Box::pin(async move {
                            capture_event_in_tx(
                                tx,
                                &stream_name,
                                &tenant,
                                &object,
                                message.payload,
                                &message.replay_id,
                            )
                            .await
                        })
                    })
                    .await?
                {
                    captured += 1;
                }
            }
            PubSubEvent::KeepAlive | PubSubEvent::Reconnected { .. } => {}
        }
    }

    Ok(captured)
}

/// Loads the stored replay cursor for a CDC stream.
///
/// # Errors
///
/// Returns an error if the checkpoint cannot be loaded or decoded.
pub async fn load_replay_id(
    store: &PgStore,
    stream_name: &str,
) -> Result<Option<ReplayId>, ForceSyncError> {
    let checkpoint = store.get_checkpoint(stream_name).await?;
    checkpoint
        .map(|checkpoint| replay_id_from_position(checkpoint.cursor_position))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── change_operation ──────────────────────────────────────────────

    #[test]
    fn change_operation_returns_delete_for_delete_change_type() {
        let payload = json!({
            "ChangeEventHeader": {
                "changeType": "DELETE"
            }
        });
        assert_eq!(change_operation(&payload), ChangeOperation::Delete);
    }

    #[test]
    fn change_operation_returns_upsert_for_create_change_type() {
        let payload = json!({
            "ChangeEventHeader": {
                "changeType": "CREATE"
            }
        });
        assert_eq!(change_operation(&payload), ChangeOperation::Upsert);
    }

    #[test]
    fn change_operation_returns_upsert_for_update_change_type() {
        let payload = json!({
            "ChangeEventHeader": {
                "changeType": "UPDATE"
            }
        });
        assert_eq!(change_operation(&payload), ChangeOperation::Upsert);
    }

    #[test]
    fn change_operation_returns_upsert_when_header_missing() {
        let payload = json!({"Name": "Acme"});
        assert_eq!(change_operation(&payload), ChangeOperation::Upsert);
    }

    #[test]
    fn change_operation_returns_upsert_when_change_type_missing() {
        let payload = json!({
            "ChangeEventHeader": {
                "entityName": "Account"
            }
        });
        assert_eq!(change_operation(&payload), ChangeOperation::Upsert);
    }

    // ── replay_id_to_position ─────────────────────────────────────────

    #[test]
    fn replay_id_to_position_single_byte() {
        let replay = ReplayId::from_bytes(vec![42]);
        let Ok(pos) = replay_id_to_position(&replay) else {
            panic!("expected Ok for single-byte replay id");
        };
        assert_eq!(pos, 42);
    }

    #[test]
    fn replay_id_to_position_multi_byte() {
        // 256 = 0x01 0x00
        let replay = ReplayId::from_bytes(vec![1, 0]);
        let Ok(pos) = replay_id_to_position(&replay) else {
            panic!("expected Ok for multi-byte replay id");
        };
        assert_eq!(pos, 256);
    }

    #[test]
    fn replay_id_to_position_full_8_bytes() {
        let replay = ReplayId::from_bytes(vec![0, 0, 0, 0, 0, 0, 1, 0]);
        let Ok(pos) = replay_id_to_position(&replay) else {
            panic!("expected Ok for 8-byte replay id");
        };
        assert_eq!(pos, 256);
    }

    #[test]
    fn replay_id_to_position_rejects_more_than_8_bytes() {
        let replay = ReplayId::from_bytes(vec![0; 9]);
        assert!(matches!(
            replay_id_to_position(&replay),
            Err(ForceSyncError::InvalidStoredValue { .. })
        ));
    }

    #[test]
    fn replay_id_to_position_rejects_value_exceeding_i64_max() {
        // 0xFF repeated 8 times = u64::MAX which exceeds i64::MAX
        let replay = ReplayId::from_bytes(vec![0xFF; 8]);
        assert!(matches!(
            replay_id_to_position(&replay),
            Err(ForceSyncError::InvalidStoredValue { .. })
        ));
    }

    // ── replay_id_from_position ───────────────────────────────────────

    #[test]
    fn replay_id_from_position_small_value() {
        let Ok(replay) = replay_id_from_position(42) else {
            panic!("expected Ok for small position");
        };
        assert_eq!(replay.as_bytes(), &[42]);
    }

    #[test]
    fn replay_id_from_position_multi_byte_value() {
        let Ok(replay) = replay_id_from_position(256) else {
            panic!("expected Ok for multi-byte position");
        };
        assert_eq!(replay.as_bytes(), &[1, 0]);
    }

    #[test]
    fn replay_id_from_position_zero() {
        let Ok(replay) = replay_id_from_position(0) else {
            panic!("expected Ok for zero position");
        };
        assert_eq!(replay.as_bytes(), &[0]);
    }

    #[test]
    fn replay_id_from_position_rejects_negative() {
        assert!(matches!(
            replay_id_from_position(-1),
            Err(ForceSyncError::InvalidStoredValue { .. })
        ));
    }

    // ── round-trip ────────────────────────────────────────────────────

    #[test]
    fn replay_id_round_trip_preserves_value() {
        let original_position: i64 = 123_456;
        let Ok(replay) = replay_id_from_position(original_position) else {
            panic!("expected Ok for from_position");
        };
        let Ok(recovered) = replay_id_to_position(&replay) else {
            panic!("expected Ok for to_position");
        };
        assert_eq!(recovered, original_position);
    }
}
