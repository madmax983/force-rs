//! Agent API streaming: Server-Sent Events (SSE) decode.
//!
//! `POST .../sessions/{id}/messages/stream` returns `text/event-stream`.
//! Typical order: `ProgressIndicator`, `TextChunk` (when chunking is on),
//! `Inform` (full message), `EndOfTurn`. On `ValidationFailureChunk`, discard
//! the chunks that you showed.

use super::types::AgentMessage;
use crate::error::{ForceError, Result};
use serde_json::Value;

/// Default size limit of one SSE event (1 MiB).
pub const DEFAULT_MAX_EVENT_BYTES: usize = 1024 * 1024;

/// One raw SSE event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseEvent {
    /// `event:` field. `None` means the default type (`message`).
    pub event: Option<String>,
    /// `data:` lines, joined with `\n`.
    pub data: String,
    /// `id:` field.
    pub id: Option<String>,
}

/// Incremental SSE decoder.
///
/// Follows the WHATWG `EventSource` rules: LF, CRLF, and CR line ends; a
/// leading BOM is removed; comment lines (`:`) are ignored; multi-line data is
/// joined; the last event ID stays until changed; an event with no blank line
/// after it is discarded. Fails if one event is larger than the limit.
#[derive(Debug)]
pub struct SseDecoder {
    line: Vec<u8>,
    pending_cr: bool,
    bom_checked: bool,
    event: Option<String>,
    data: String,
    has_data: bool,
    id: Option<String>,
    max_event_bytes: usize,
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_EVENT_BYTES)
    }
}

impl SseDecoder {
    /// Creates a decoder with a size limit per event.
    #[must_use]
    pub(crate) const fn new(max_event_bytes: usize) -> Self {
        Self {
            line: Vec::new(),
            pending_cr: false,
            bom_checked: false,
            event: None,
            data: String::new(),
            has_data: false,
            id: None,
            max_event_bytes,
        }
    }

    /// Adds bytes and returns the events that are complete.
    ///
    /// # Errors
    ///
    /// Returns [`ForceError::InvalidInput`] if an event is larger than the limit.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>> {
        let mut out = Vec::new();
        for &byte in chunk {
            if self.pending_cr {
                self.pending_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\n' => self.end_line(&mut out)?,
                b'\r' => {
                    self.end_line(&mut out)?;
                    self.pending_cr = true;
                }
                _ => {
                    self.line.push(byte);
                    if self.line.len() + self.data.len() > self.max_event_bytes {
                        return Err(too_large(self.max_event_bytes));
                    }
                }
            }
        }
        Ok(out)
    }

    /// Handles one complete line.
    fn end_line(&mut self, out: &mut Vec<SseEvent>) -> Result<()> {
        let mut line = std::mem::take(&mut self.line);
        if !self.bom_checked {
            self.bom_checked = true;
            if line.starts_with(UTF8_BOM) {
                line.drain(..UTF8_BOM.len());
            }
        }
        if line.is_empty() {
            self.dispatch(out);
            return Ok(());
        }
        if line[0] == b':' {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&line);
        let (field, value) = match text.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (text.as_ref(), ""),
        };
        match field {
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
                if self.data.len() > self.max_event_bytes {
                    return Err(too_large(self.max_event_bytes));
                }
            }
            "event" => self.event = (!value.is_empty()).then(|| value.to_owned()),
            "id" if !value.contains('\0') => self.id = Some(value.to_owned()),
            _ => {}
        }
        Ok(())
    }

    /// Emits the buffered event if it has data, then resets the buffer.
    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        let event = self.event.take();
        if std::mem::take(&mut self.has_data) {
            out.push(SseEvent {
                event,
                data: std::mem::take(&mut self.data),
                id: self.id.clone(),
            });
        }
    }
}

/// Kind of a streamed agent event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentStreamEventKind {
    /// The agent is working.
    ProgressIndicator,
    /// Part of the reply text.
    TextChunk,
    /// The full reply, with citations.
    Inform,
    /// The turn is complete.
    EndOfTurn,
    /// Discard the chunks that you showed.
    ValidationFailureChunk,
    /// An error event.
    Error,
    /// A type that this crate does not know.
    Other(String),
}

impl AgentStreamEventKind {
    /// Maps a type name. Case and `_` are ignored, so `TEXT_CHUNK` and
    /// `TextChunk` are the same.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        let key: String = name
            .chars()
            .filter(|c| *c != '_')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        match key.as_str() {
            "progressindicator" => Self::ProgressIndicator,
            "textchunk" => Self::TextChunk,
            "inform" => Self::Inform,
            "endofturn" => Self::EndOfTurn,
            "validationfailurechunk" => Self::ValidationFailureChunk,
            "error" => Self::Error,
            _ => Self::Other(name.to_owned()),
        }
    }
}

/// One streamed agent event.
#[derive(Debug, Clone)]
pub struct AgentStreamEvent {
    /// Event kind. From `message.type`, else from the SSE `event:` name.
    pub kind: AgentStreamEventKind,
    /// The decoded `message` object, if present.
    pub message: Option<AgentMessage>,
    /// The full `data` payload. A string if it is not JSON.
    pub data: Value,
    /// The SSE `event:` name.
    pub event_name: Option<String>,
}

impl AgentStreamEvent {
    /// Builds an agent event from a raw SSE event.
    #[must_use]
    pub(crate) fn from_sse(event: SseEvent) -> Self {
        let data = serde_json::from_str::<Value>(&event.data).unwrap_or(Value::String(event.data));
        let raw = data.get("message").filter(|m| m.is_object());
        let message_type = raw
            .and_then(|m| m.get("type"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        // Decode with a default type, so a message without `type` keeps its text.
        let message = raw.and_then(|m| {
            let mut m = m.clone();
            if let Some(obj) = m.as_object_mut() {
                obj.entry("type")
                    .or_insert_with(|| event.event.clone().unwrap_or_default().into());
            }
            serde_json::from_value::<AgentMessage>(m).ok()
        });
        let kind = message_type
            .as_deref()
            .or(event.event.as_deref())
            .map_or_else(
                || AgentStreamEventKind::Other(String::new()),
                AgentStreamEventKind::from_name,
            );
        Self {
            kind,
            message,
            data,
            event_name: event.event,
        }
    }

    /// Returns the message text, if any.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.message.as_ref().and_then(|m| m.message.as_deref())
    }

    /// Returns true for `EndOfTurn`.
    #[must_use]
    pub fn is_end_of_turn(&self) -> bool {
        self.kind == AgentStreamEventKind::EndOfTurn
    }
}

/// Stream of agent events from [`send_message_stream`](super::AgentHandler::send_message_stream).
pub type AgentEventStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<AgentStreamEvent>> + Send>>;

/// Decodes a byte stream into agent events.
///
/// The stream ends after the first error.
pub fn sse_event_stream<S>(bytes: S, max_event_bytes: usize) -> AgentEventStream
where
    S: futures::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    struct State<S> {
        bytes: std::pin::Pin<Box<S>>,
        decoder: SseDecoder,
        queue: std::collections::VecDeque<SseEvent>,
        done: bool,
    }

    let state = State {
        bytes: Box::pin(bytes),
        decoder: SseDecoder::new(max_event_bytes),
        queue: std::collections::VecDeque::new(),
        done: false,
    };
    Box::pin(futures::stream::unfold(state, |mut st| async move {
        use futures::StreamExt;
        loop {
            if let Some(event) = st.queue.pop_front() {
                return Some((Ok(AgentStreamEvent::from_sse(event)), st));
            }
            if st.done {
                return None;
            }
            match st.bytes.next().await {
                Some(Ok(chunk)) => match st.decoder.push(&chunk) {
                    Ok(events) => st.queue.extend(events),
                    Err(err) => {
                        st.done = true;
                        return Some((Err(err), st));
                    }
                },
                Some(Err(err)) => {
                    st.done = true;
                    return Some((Err(crate::error::HttpError::from(err).into()), st));
                }
                None => st.done = true,
            }
        }
    }))
}

/// UTF-8 byte order mark.
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Error for an event larger than the limit.
fn too_large(limit: usize) -> ForceError {
    crate::error::HttpError::PayloadTooLarge { limit_bytes: limit }.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use proptest::prelude::*;

    fn decode_all(input: &[u8]) -> Vec<SseEvent> {
        SseDecoder::default().push(input).must()
    }

    #[test]
    fn test_basic_event_and_comment() {
        let events = decode_all(b": keep-alive\nevent: TEXT_CHUNK\nid: 7\ndata: {\"a\":1}\n\n");
        assert_eq!(
            events,
            vec![SseEvent {
                event: Some("TEXT_CHUNK".into()),
                data: "{\"a\":1}".into(),
                id: Some("7".into()),
            }]
        );
    }

    #[test]
    fn test_multiline_data_and_crlf() {
        let events = decode_all(b"data: line1\r\ndata:line2\r\n\r\ndata: x\rdata: y\r\r");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "line1\nline2");
        assert_eq!(events[1].data, "x\ny");
    }

    #[test]
    fn test_event_without_data_is_not_dispatched() {
        assert_eq!(decode_all(b"event: ping\n\n").len(), 0);
    }

    #[test]
    fn test_split_chunks_including_crlf_boundary() {
        let mut d = SseDecoder::default();
        assert_eq!(d.push(b"data: hel").must().len(), 0);
        assert_eq!(d.push(b"lo\r").must().len(), 0);
        let out = d.push(b"\n\r\n").must();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data, "hello");
    }

    #[test]
    fn test_unterminated_event_is_not_dispatched() {
        // WHATWG: an event without a blank line after it is discarded at EOF.
        assert_eq!(decode_all(b"data: tail").len(), 0);
        assert_eq!(decode_all(b"data: tail\n").len(), 0);
    }

    #[test]
    fn test_leading_bom_is_stripped() {
        let events = decode_all(b"\xEF\xBB\xBFdata: x\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "x");
    }

    #[test]
    fn test_bom_split_across_chunks() {
        let mut d = SseDecoder::default();
        assert_eq!(d.push(b"\xEF\xBB").must().len(), 0);
        let out = d.push(b"\xBFdata: y\n\n").must();
        assert_eq!(out[0].data, "y");
    }

    #[test]
    fn test_empty_event_name_means_default_type() {
        let events = decode_all(b"event:\ndata: x\n\n");
        assert_eq!(events[0].event, None);
    }

    #[test]
    fn test_last_event_id_persists_and_nul_id_is_ignored() {
        let events = decode_all(b"id: 1\ndata: a\n\ndata: b\n\nid: 2\0x\ndata: c\n\n");
        let ids: Vec<_> = events.iter().map(|e| e.id.as_deref()).collect();
        assert_eq!(ids, vec![Some("1"), Some("1"), Some("1")]);
    }

    #[test]
    fn test_field_without_colon_and_lone_cr_across_chunks() {
        let mut d = SseDecoder::default();
        assert_eq!(d.push(b"data\r").must().len(), 0);
        let out = d.push(b"data: z\r\r").must();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data, "\nz");
    }

    #[test]
    fn test_event_size_limit() {
        let mut d = SseDecoder::new(16);
        let err = d.push(b"data: 0123456789abcdefXYZ").err().must();
        assert!(
            matches!(
                err,
                ForceError::Http(crate::error::HttpError::PayloadTooLarge { limit_bytes: 16 })
            ),
            "{err:?}"
        );

        let mut d = SseDecoder::new(16);
        assert!(d.push(b"data: 0123456789\n").is_ok());
        assert!(d.push(b"data: 0123456789\n").is_err());
    }

    #[test]
    fn test_kind_from_name_normalizes() {
        use AgentStreamEventKind as K;
        assert_eq!(K::from_name("TextChunk"), K::TextChunk);
        assert_eq!(K::from_name("TEXT_CHUNK"), K::TextChunk);
        assert_eq!(K::from_name("progress_indicator"), K::ProgressIndicator);
        assert_eq!(K::from_name("Inform"), K::Inform);
        assert_eq!(K::from_name("END_OF_TURN"), K::EndOfTurn);
        assert_eq!(
            K::from_name("ValidationFailureChunk"),
            K::ValidationFailureChunk
        );
        assert_eq!(K::from_name("ERROR"), K::Error);
        assert_eq!(K::from_name("Escalate"), K::Other("Escalate".into()));
    }

    #[test]
    fn test_agent_event_prefers_message_type() {
        let ev = AgentStreamEvent::from_sse(SseEvent {
            event: Some("UNKNOWN_NAME".into()),
            data: r#"{"offset":0,"message":{"type":"TextChunk","id":"m1","message":"Hel"}}"#.into(),
            id: None,
        });
        assert_eq!(ev.kind, AgentStreamEventKind::TextChunk);
        assert_eq!(ev.text(), Some("Hel"));
        assert_eq!(ev.data["offset"], 0);
    }

    #[test]
    fn test_agent_event_falls_back_to_event_name_and_raw_text() {
        let ev = AgentStreamEvent::from_sse(SseEvent {
            event: Some("END_OF_TURN".into()),
            data: "not json".into(),
            id: None,
        });
        assert!(ev.is_end_of_turn());
        assert_eq!(ev.data, Value::String("not json".into()));
        assert!(ev.message.is_none());
    }

    #[test]
    fn test_no_message_and_no_event_name_is_other_empty() {
        let ev = AgentStreamEvent::from_sse(SseEvent {
            event: None,
            data: r#"{"offset":1}"#.into(),
            id: None,
        });
        assert_eq!(ev.kind, AgentStreamEventKind::Other(String::new()));
    }

    #[test]
    fn test_untyped_message_still_gives_kind_and_text() {
        // `result` is an object and `citedReferences` is null. The text must survive.
        let ev = AgentStreamEvent::from_sse(SseEvent {
            event: Some("INFORM".into()),
            data: r#"{"message":{"type":"Inform","message":"Hi","result":{"a":1},"citedReferences":null}}"#
                .into(),
            id: None,
        });
        assert_eq!(ev.kind, AgentStreamEventKind::Inform);
        assert_eq!(ev.text(), Some("Hi"));
        let msg = ev.message.must();
        assert_eq!(msg.result.len(), 1);
        assert_eq!(msg.cited_references.len(), 0);
    }

    #[test]
    fn test_message_without_type_uses_event_name() {
        let ev = AgentStreamEvent::from_sse(SseEvent {
            event: Some("TEXT_CHUNK".into()),
            data: r#"{"message":{"message":"Hel"}}"#.into(),
            id: None,
        });
        assert_eq!(ev.kind, AgentStreamEventKind::TextChunk);
        assert_eq!(ev.text(), Some("Hel"));
    }

    proptest! {
        // Any set of splits of the byte stream gives the same events.
        #[test]
        fn prop_split_invariance(mut splits in proptest::collection::vec(0usize..80, 0..6)) {
            let input: &[u8] = b"\xEF\xBB\xBFevent: a\ndata: one\n\n: c\r\ndata: two\rdata: three\r\n\r\nid: 9\ndata: four\n\ndata: tail";
            for s in &mut splits {
                *s = (*s).min(input.len());
            }
            splits.sort_unstable();
            let mut d = SseDecoder::default();
            let mut got = Vec::new();
            let mut start = 0;
            for at in splits {
                got.extend(d.push(&input[start..at]).must());
                start = at;
            }
            got.extend(d.push(&input[start..]).must());
            prop_assert_eq!(got, decode_all(input));
            prop_assert_eq!(decode_all(input).len(), 3);
        }

        // The decoder never panics on arbitrary bytes.
        #[test]
        fn prop_no_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let mut d = SseDecoder::new(256);
            let _ = d.push(&bytes);
        }
    }
}
