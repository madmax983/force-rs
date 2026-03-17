//! Domain types for Salesforce Pub/Sub API events and responses.

/// Opaque replay cursor. Consumers store and pass back; never interpret the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayId(pub(crate) Vec<u8>);

impl ReplayId {
    /// Construct from raw bytes (e.g., from a FetchResponse).
    #[must_use]
    pub const fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Return the raw bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// True if this replay ID has no bytes (represents "no replay").
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A decoded event received from the subscribe stream.
#[derive(Debug, Clone)]
pub struct EventMessage<T> {
    /// Decoded event payload.
    pub payload: T,
    /// Replay cursor for this event — store to resume from here.
    pub replay_id: ReplayId,
    /// Avro schema ID used to decode this event.
    pub schema_id: String,
    /// Unique event identifier.
    pub event_id: String,
}

/// Items yielded by the subscribe stream.
#[derive(Debug)]
pub enum PubSubEvent<T> {
    /// A decoded event.
    Event(EventMessage<T>),
    /// Stream was reconnected after a drop.
    Reconnected {
        /// The replay ID we resumed from.
        replay_id: ReplayId,
        /// Which reconnection attempt this was (1-indexed).
        attempt: u32,
    },
    /// Salesforce heartbeat — no events this batch.
    KeepAlive,
}

/// Per-event result from a publish operation.
#[derive(Debug, Clone)]
pub struct PublishResult {
    /// Replay ID assigned by Salesforce (if successful).
    pub replay_id: Option<ReplayId>,
    /// Error message if this individual event failed.
    pub error: Option<String>,
}

impl PublishResult {
    /// Returns true if this event was published successfully.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

/// Response from a publish operation.
#[derive(Debug, Clone)]
pub struct PublishResponse {
    /// The topic name events were published to.
    pub topic_name: String,
    /// Per-event results (same order as the input events).
    pub results: Vec<PublishResult>,
}

impl PublishResponse {
    /// Returns true if all events were published successfully.
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(PublishResult::is_success)
    }

    /// Returns the number of failed events.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| !r.is_success()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_id_from_bytes_roundtrip() {
        let bytes = vec![1u8, 2, 3, 4];
        let id = ReplayId::from_bytes(bytes.clone());
        assert_eq!(id.as_bytes(), bytes.as_slice());
    }

    #[test]
    fn test_replay_id_empty() {
        let id = ReplayId::from_bytes(vec![]);
        assert!(id.is_empty());
    }

    #[test]
    fn test_replay_id_not_empty() {
        let id = ReplayId::from_bytes(vec![1, 2, 3]);
        assert!(!id.is_empty());
    }

    #[test]
    fn test_publish_result_success() {
        let r = PublishResult {
            replay_id: Some(ReplayId::from_bytes(vec![1])),
            error: None,
        };
        assert!(r.is_success());
    }

    #[test]
    fn test_publish_result_failure() {
        let r = PublishResult {
            replay_id: None,
            error: Some("INVALID_TYPE".to_string()),
        };
        assert!(!r.is_success());
    }

    #[test]
    fn test_publish_response_all_succeeded() {
        let resp = PublishResponse {
            topic_name: "/event/MyEvent__e".to_string(),
            results: vec![
                PublishResult {
                    replay_id: Some(ReplayId::from_bytes(vec![1])),
                    error: None,
                },
                PublishResult {
                    replay_id: Some(ReplayId::from_bytes(vec![2])),
                    error: None,
                },
            ],
        };
        assert!(resp.all_succeeded());
        assert_eq!(resp.failure_count(), 0);
    }

    #[test]
    fn test_publish_response_partial_failure() {
        let resp = PublishResponse {
            topic_name: "/event/MyEvent__e".to_string(),
            results: vec![
                PublishResult {
                    replay_id: Some(ReplayId::from_bytes(vec![1])),
                    error: None,
                },
                PublishResult {
                    replay_id: None,
                    error: Some("ERR".to_string()),
                },
            ],
        };
        assert!(!resp.all_succeeded());
        assert_eq!(resp.failure_count(), 1);
    }
}
