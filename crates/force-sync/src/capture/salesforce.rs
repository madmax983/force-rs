//! Stream-driven Salesforce CDC capture.

use force_pubsub::{PubSubEvent, ReplayId};
use futures::{Stream, StreamExt};
use serde_json::Value;
use tokio_postgres::GenericClient;

use crate::{
    config::ObjectSync,
    error::ForceSyncError,
    identity::SyncKey,
    model::{ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
    store::pg::{AppendResult, PgStore},
};

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
    let cursor = envelope
        .cursor()
        .ok_or(ForceSyncError::MissingSourceCursor)?
        .as_db_value();

    let inserted = match PgStore::append_journal_if_new_in_tx(client, &envelope).await? {
        AppendResult::Inserted { journal_id } => {
            PgStore::enqueue_apply_task_in_tx(client, journal_id, 0).await?;
            true
        }
        AppendResult::Duplicate => false,
    };

    PgStore::advance_checkpoint_if_greater_in_tx(client, stream_name, replay_position, &cursor)
        .await?;
    Ok(inserted)
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
