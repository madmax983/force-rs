# Vantage Spec: OAuth 2.0 Device Flow

👤 **User Story:** "As a CLI tool builder, I want to authenticate users securely without requiring them to copy-paste passwords or handle complex callback servers, so that they can easily grant access from an environment with limited or no browser capability (e.g. headless servers, CI runners)."

✅ **Acceptance Criteria:**
- Must implement the Salesforce OAuth 2.0 Device Flow specification.
- Must provide a polling mechanism that waits for the user to approve the device code on another device.
- Must handle all standard Device Flow error responses (e.g., `authorization_pending`, `slow_down`, `expired_token`, `access_denied`).
- Must not require a local callback server or user input of credentials directly into the terminal.
- Must return a standard `AccessToken` upon successful completion.

🚫 **Out of Scope:**
- Interactive terminal UI components (e.g. rendering QR codes). The library should return the verification URL and code for the consumer to display however they see fit.
