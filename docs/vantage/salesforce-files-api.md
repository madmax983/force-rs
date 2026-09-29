# 🔭 Vantage: Spec for Streaming Salesforce Files I/O

> **Status:** the `files` feature shipped in v0.2.0 — `client.files()` already
> covers upload/download/link-to-record; see
> [`docs/guide/surfaces/files.md`](../guide/surfaces/files.md). What's below
> is the part of the original spec that did **not** ship: streaming transfer.
> The shipped handler buffers whole bodies in memory (capped at 100 MiB /
> 10 MiB; see `files.md` and [ADR-004](../adr/004-feature-gates.md)) and is
> not a chunked/streaming transfer.

**What business problem does this solve?**
Enterprise Salesforce integrations frequently need to upload, download, and manage documents (e.g., invoices, signed contracts, or product images). The shipped `files` handler solves the ergonomics problem (no manual base64/multipart handling) but still loads the whole body into memory, which caps usable file size and wastes memory for larger documents.

**Gap Analysis:**
The shipped `client.files()` handler does not stream large binary files to and from Salesforce. Files encoded as base64 in a standard JSON payload, or buffered whole in memory, consume excessive memory relative to file size. A stream-native path (an `AsyncRead`/`AsyncWrite`-based upload/download) would remove that ceiling without changing the ergonomics the shipped handler already provides.

👤 **User Story:**
As an Integration Developer, I want to stream binary files into and out of Salesforce Files, so that I can upload and download large documents without the peak memory footprint scaling with file size.

✅ **Acceptance Criteria:**
- Must support streaming file uploads (creating `ContentVersion`) directly from a file path or an async reader to avoid loading the entire file into memory.
- Must support streaming file downloads from a `ContentVersion` ID directly to a file path or async writer.
- Must coexist with the shipped in-memory `upload`/`download`/`link_to_record` methods rather than replacing them (small-file callers shouldn't have to opt into streaming complexity).
- Success = Ability to upload and download a file larger than the current 100 MiB in-memory cap with peak memory usage well under the file size.

🚫 **Out of Scope:**
- Handling legacy Salesforce Attachments (only modern Salesforce Files / ContentVersion are supported).
- Document versioning conflict resolution (assumes standard append/new version logic).
- Re-implementing `link_to_record`, which already ships and needs no streaming variant.
