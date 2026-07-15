# 🔭 Vantage: Spec for OAuth 2.0 Device Flow

**Business problem:**
CLI tools and headless applications (like background workers or CI/CD pipelines running on devices without a web browser) need to authenticate with Salesforce securely. Traditional OAuth web server flows require a local browser redirect, which isn't possible in these environments. Asking users to manually copy-paste tokens or handle raw credentials is insecure and error-prone.

**Gap Analysis:**
The current `force-rs` auth module supports `ClientCredentials`, `JwtBearer`, and standard web-based flows, but lacks support for RFC 8628 (OAuth 2.0 Device Authorization Grant). This limits the ability to use `force-rs` in interactive CLI tooling where a user needs to authorize the app via their smartphone or desktop browser.

**Success metric:**
Success = A user can initiate the device flow via `force-rs`, receive a user code and verification URL, complete the login on a separate device, and `force-rs` successfully polls and retrieves the access token within the expiration window.

👤 **User Story:**
As a CLI tool developer, I want to authenticate users using the OAuth 2.0 Device Flow, so that users can securely log in via a secondary device (like their smartphone or desktop browser) without me having to handle their credentials or run a local web server.

✅ **Acceptance Criteria:**
- Must provide an API to initiate the flow, returning the `verification_uri`, `user_code`, and `device_code`.
- Must automatically poll the Salesforce token endpoint at the recommended interval.
- Must handle `authorization_pending` and `slow_down` error responses gracefully during polling.
- Must return the access token and refresh token once the user successfully authenticates.
- Must timeout gracefully if the user does not authenticate within the provided expiration time.

🚫 **Out of Scope:**
- Storing or caching the resulting tokens securely (the caller must handle token storage).
- Supporting legacy Username/Password flows.
