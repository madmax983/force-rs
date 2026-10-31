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
    store::pg::{AppendResult, DeadLetter, PgStore, record_map},
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

/// How long the first window of a [`CdcStart::Latest`] stream with no
/// checkpoint waits for a position to resume from.
///
/// Salesforce sends a keepalive with the latest replay ID when a channel has
/// had no event for about 270 seconds.
pub const FIRST_POSITION_WAIT: Duration = Duration::from_secs(300);

/// Stores the latest replay ID of a keepalive as the stream checkpoint.
///
/// A keepalive follows every event before it, and the window commits events
/// in order. So no event at or before this position is still uncommitted.
async fn checkpoint_keepalive(
    store: &PgStore,
    stream_name: &str,
    latest: &ReplayId,
) -> Result<(), ForceSyncError> {
    let position = replay_id_to_position(latest)?;
    let cursor = SourceCursor::SalesforceReplayId(position).as_db_value();
    store
        .advance_checkpoint_if_greater(stream_name, position, cursor)
        .await?;
    Ok(())
}

/// Whether `error` comes from the content of one event.
///
/// Such an event can never be captured, so it is quarantined. A
/// configuration or database error fails the window instead.
const fn event_content_error(error: &ForceSyncError) -> bool {
    matches!(
        error,
        ForceSyncError::UncapturableCdcEvent { .. } | ForceSyncError::EmptySyncKeyPart { .. }
    )
}

const fn uncapturable(reason: String) -> ForceSyncError {
    ForceSyncError::UncapturableCdcEvent { reason }
}

/// A change event split into the parts that capture uses.
#[derive(Debug, Clone, PartialEq)]
struct CdcEvent {
    operation: ChangeOperation,
    /// The one record that the event names, from `ChangeEventHeader.recordIds`.
    record_id: Option<String>,
    /// The record fields, without the header.
    body: Value,
}

/// Reads a change event payload.
///
/// The header leaves the body. An update or delete event names its changed
/// fields only, and every other field is null. So the nulls of such an event
/// are dropped, and a null never overwrites a known value. A create keeps
/// its nulls. The record ID joins the body as `Id`, so the applied change
/// projects it onto the sync link.
///
/// # Errors
///
/// Returns [`ForceSyncError::UncapturableCdcEvent`] for a gap event, which
/// carries no field values, and for an event that names several records.
fn read_cdc_event(mut payload: Value) -> Result<CdcEvent, ForceSyncError> {
    let header = payload
        .as_object_mut()
        .and_then(|body| body.remove("ChangeEventHeader"))
        .unwrap_or(Value::Null);
    let change_type = header
        .get("changeType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if change_type.starts_with("GAP_") {
        return Err(uncapturable(format!(
            "a {change_type} event carries no field values, so reconcile the record"
        )));
    }

    let record_ids: Vec<&str> = header
        .get("recordIds")
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if record_ids.len() > 1 {
        return Err(uncapturable(format!(
            "the event names {} records, and capture takes one record per event",
            record_ids.len()
        )));
    }
    let record_id = record_ids.first().map(|id| (*id).to_owned());

    let operation = if change_type == "DELETE" {
        ChangeOperation::Delete
    } else {
        ChangeOperation::Upsert
    };
    if matches!(change_type, "UPDATE" | "DELETE") {
        strip_nulls(&mut payload);
    }
    if let (Some(id), Some(body)) = (&record_id, payload.as_object_mut()) {
        body.entry("Id")
            .or_insert_with(|| Value::String(id.clone()));
    }

    Ok(CdcEvent {
        operation,
        record_id,
        body: payload,
    })
}

/// Drops every null field, and every object that has no field left.
fn strip_nulls(value: &mut Value) {
    if let Value::Object(fields) = value {
        fields.retain(|_, field| {
            strip_nulls(field);
            !field.is_null() && !matches!(field, Value::Object(inner) if inner.is_empty())
        });
    }
}

/// Finds the external ID that keys the record of `event`.
///
/// The event body has it when the field is set or changed. A sparse event
/// has only the record ID, so the key comes from an earlier event of the
/// record or from its sync link. An event body that has both values
/// updates the record map.
async fn resolve_external_id_in_tx<C>(
    client: &C,
    tenant: &str,
    object: &ObjectSync,
    event: &CdcEvent,
) -> Result<String, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let field = object
        .external_id_field()
        .ok_or(ForceSyncError::MissingConfiguration {
            field: "external_id_field",
        })?;
    let in_body = event
        .body
        .get(field)
        .and_then(Value::as_str)
        .filter(|external_id| !external_id.is_empty());

    match (in_body, event.record_id.as_deref()) {
        (Some(external_id), Some(record_id)) => {
            record_map::remember_salesforce_record_in_tx(
                client,
                tenant,
                object.object_name(),
                record_id,
                external_id,
            )
            .await?;
            Ok(external_id.to_owned())
        }
        (Some(external_id), None) => Ok(external_id.to_owned()),
        (None, Some(record_id)) => record_map::external_id_for_salesforce_record_in_tx(
            client,
            tenant,
            object.object_name(),
            record_id,
        )
        .await?
        .ok_or_else(|| {
            uncapturable(format!(
                "the event has no {field} value, and no earlier event or sync link knows record {record_id}"
            ))
        }),
        (None, None) => Err(uncapturable(format!(
            "the event has no {field} value and no record ID"
        ))),
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

/// Builds the journal envelope of one change event.
async fn build_envelope_in_tx<C>(
    client: &C,
    tenant: &str,
    object: &ObjectSync,
    payload: Value,
    replay_position: i64,
) -> Result<ChangeEnvelope, ForceSyncError>
where
    C: GenericClient + Sync + ?Sized,
{
    let event = read_cdc_event(payload)?;
    let external_id = resolve_external_id_in_tx(client, tenant, object, &event).await?;

    Ok(ChangeEnvelope::new(
        SyncKey::new(
            tenant.to_owned(),
            object.object_name().to_owned(),
            external_id,
        )?,
        SourceSystem::Salesforce,
        event.operation,
        chrono::Utc::now(),
        event.body,
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
    let envelope = build_envelope_in_tx(client, tenant, object, payload, replay_position).await?;
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
    let outcome = match build_envelope_in_tx(
        client,
        stream.tenant(),
        object,
        payload.clone(),
        replay_position,
    )
    .await
    {
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
/// after `window.max_events` events, or when the source ends the stream. A
/// keepalive with a replay ID moves the checkpoint to that ID. The first
/// window of a [`CdcStart::Latest`] stream with no checkpoint stays open
/// past `window.max_duration`, up to [`FIRST_POSITION_WAIT`], until an event
/// or a keepalive gives it a position, so the next window cannot skip
/// events. Each
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

    let checkpoint = load_replay_id(store, stream.name()).await?;
    // Only a Latest stream with no checkpoint has no position to resume
    // from. Earliest always replays the retention window again.
    let mut resumable = checkpoint.is_some() || stream.start() == CdcStart::Earliest;
    let replay = checkpoint.map_or_else(
        || match stream.start() {
            CdcStart::Latest => ReplayPreset::Latest,
            CdcStart::Earliest => ReplayPreset::Earliest,
        },
        ReplayPreset::Custom,
    );
    let mut events = source.subscribe(stream.topic(), replay).await?;
    let opened = tokio::time::Instant::now();
    let deadline = opened + window.max_duration;
    // A window that ends with no position makes the next one subscribe at
    // Latest again, and the events between the two are lost. So the window
    // stays open until the first event or keepalive gives it a position.
    let hold_until = opened + window.max_duration.max(FIRST_POSITION_WAIT);
    let mut report = CdcWindowReport {
        captured: 0,
        duplicates: 0,
        quarantined: 0,
        end: CdcWindowEnd::Deadline,
    };
    let mut read = 0usize;

    while read < window.max_events {
        let until = if resumable { deadline } else { hold_until };
        let Ok(next) = tokio::time::timeout_at(until, events.next()).await else {
            report.end = CdcWindowEnd::Deadline;
            return Ok(report);
        };
        let Some(item) = next else {
            report.end = CdcWindowEnd::StreamEnded;
            return Ok(report);
        };
        let message = match item? {
            PubSubEvent::Event(message) => message,
            PubSubEvent::KeepAlive {
                latest_replay_id: Some(latest),
            } => {
                checkpoint_keepalive(store, stream.name(), &latest).await?;
                resumable = true;
                continue;
            }
            PubSubEvent::KeepAlive {
                latest_replay_id: None,
            }
            | PubSubEvent::Reconnected { .. } => continue,
        };
        resumable = true;
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
            PubSubEvent::KeepAlive {
                latest_replay_id: Some(latest),
            } => checkpoint_keepalive(store, stream_name, &latest).await?,
            PubSubEvent::KeepAlive {
                latest_replay_id: None,
            }
            | PubSubEvent::Reconnected { .. } => {}
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

    // ── read_cdc_event ────────────────────────────────────────────────

    fn read(payload: &Value) -> CdcEvent {
        read_cdc_event(payload.clone()).unwrap_or_else(|error| panic!("read: {error}"))
    }

    fn operation_of(change_type: &str) -> ChangeOperation {
        read(&json!({"ChangeEventHeader": {"changeType": change_type}})).operation
    }

    #[test]
    fn a_delete_event_reads_as_a_delete() {
        assert_eq!(operation_of("DELETE"), ChangeOperation::Delete);
    }

    #[test]
    fn create_update_and_undelete_events_read_as_upserts() {
        for change_type in ["CREATE", "UPDATE", "UNDELETE"] {
            assert_eq!(operation_of(change_type), ChangeOperation::Upsert);
        }
    }

    #[test]
    fn an_event_with_no_header_or_change_type_reads_as_an_upsert() {
        assert_eq!(
            read(&json!({"Name": "Acme"})).operation,
            ChangeOperation::Upsert
        );
        assert_eq!(
            read(&json!({"ChangeEventHeader": {"entityName": "Account"}})).operation,
            ChangeOperation::Upsert
        );
    }

    #[test]
    fn the_header_leaves_the_body_and_the_record_id_joins_it() {
        let event = read(&json!({
            "ChangeEventHeader": {"changeType": "CREATE", "recordIds": ["001A"]},
            "Name": "Acme",
            "Phone": null,
        }));
        assert_eq!(event.record_id.as_deref(), Some("001A"));
        assert_eq!(
            event.body,
            json!({"Id": "001A", "Name": "Acme", "Phone": null})
        );
    }

    #[test]
    fn an_update_drops_unchanged_nulls_at_every_level() {
        let event = read(&json!({
            "ChangeEventHeader": {"changeType": "UPDATE", "recordIds": ["003A"]},
            "Name": {"FirstName": null, "LastName": "Doe", "Salutation": null},
            "MailingAddress": {"City": null, "Street": null},
            "Phone": null,
            "Tags": [null, "vip"],
        }));
        assert_eq!(
            event.body,
            json!({"Id": "003A", "Name": {"LastName": "Doe"}, "Tags": [null, "vip"]})
        );
    }

    #[test]
    fn a_payload_id_is_kept_over_the_header_record_id() {
        let event = read(&json!({
            "ChangeEventHeader": {"changeType": "UPDATE", "recordIds": ["001A"]},
            "Id": "001B",
        }));
        assert_eq!(event.body["Id"], json!("001B"));
    }

    #[test]
    fn gap_and_multi_record_events_are_uncapturable() {
        for payload in [
            json!({"ChangeEventHeader": {"changeType": "GAP_OVERFLOW"}}),
            json!({"ChangeEventHeader": {"changeType": "UPDATE", "recordIds": ["001A", "001B"]}}),
        ] {
            let Err(error) = read_cdc_event(payload) else {
                panic!("the event must be uncapturable");
            };
            assert!(event_content_error(&error), "{error}");
        }
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
