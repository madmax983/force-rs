# 🔭 Vantage: Spec for Salesforce Files API

**What business problem does this solve?**
Enterprise Salesforce integrations frequently need to upload, download, and manage documents (e.g., invoices, signed contracts, or product images). Interacting with Salesforce Files (ContentVersion, ContentDocument, and ContentDocumentLink) via the standard REST CRUD API is unnecessarily complex, requiring developers to manually handle base64 encoding/decoding, multipart form boundaries, and relationship linkage logic.

**Gap Analysis:**
The standard `force` crate `rest` CRUD module does not seamlessly handle streaming large binary files to and from Salesforce. Large files encoded as base64 in a standard JSON payload consume excessive memory and often hit Salesforce REST API limits. We need a dedicated, stream-native API surface that manages multipart uploads and downloads without blowing up the memory footprint.

👤 **User Story:**
As an Integration Developer, I want a dedicated API to stream binary files into and out of Salesforce Files, so that I can upload and download large documents efficiently without writing manual base64 conversions or complex multipart HTTP requests.

✅ **Acceptance Criteria:**
- Must provide a dedicated `files` module (e.g., `client.files()`).
- Must support streaming file uploads (creating `ContentVersion`) directly from a file path or an async reader to avoid loading the entire file into memory.
- Must support streaming file downloads from a `ContentVersion` ID directly to a file path or async writer.
- Must provide a convenience method to automatically link a newly uploaded file to an existing Salesforce record (creating the `ContentDocumentLink`).
- Success = Ability to upload and download a 10MB PDF file to an Account record with less than 20MB of peak memory usage and under 5 lines of Rust code.

🚫 **Out of Scope:**
- Handling legacy Salesforce Attachments (only modern Salesforce Files / ContentVersion are supported).
- Document versioning conflict resolution (assumes standard append/new version logic).
