# 🔭 Vantage: Spec for OAuth 2.0 Device Flow

## Overview
A specification to implement the OAuth 2.0 Device Authorization Grant (Device Flow) for Salesforce authentication.

## 👤 User Story
As a CLI Developer or IoT Device Builder, I want to authenticate users securely without requiring a local web server or a browser on the device, so that users can authorize the application from a separate device (like their phone or laptop).

## The "So What?"
**What business problem does this solve?**
Many modern tools (like CLIs, CI/CD runners, and IoT devices) run in environments where launching a browser or listening on a local port is impossible or insecure. The OAuth 2.0 Device Flow solves this by giving the user a code to enter on a separate, browser-equipped device. Supporting this natively in `force-rs` enables developers to build secure, headless tools and CLI applications without forcing them to hand-roll the polling and token exchange logic.

## 📈 Success Metrics
- **Success =** A developer can initiate the flow, display the user code, and successfully poll for the access token with less than 10 lines of code.
- **Adoption:** CLI tools built with `force-rs` migrate to Device Flow from legacy Username/Password flows.
- **Reliability:** 100% correct handling of Salesforce's `authorization_pending`, `slow_down`, and timeout errors during the polling phase.

## Gap Analysis
Currently, `force-rs` supports Client Credentials, JWT Bearer, and legacy Username/Password. For interactive CLI tools, developers must either use the insecure legacy password flow (which fails with MFA) or implement a complex local web server for the Web Server flow. The Device Flow is the industry standard for this use case, but without built-in support, developers must manually manage the initial request, display the code, and implement the precise polling logic (including handling `slow_down` responses).

## ✅ Acceptance Criteria
- **Initiation:** Must be able to request a device code, user code, and verification URI from Salesforce.
- **Polling:** Must provide an async mechanism to poll the token endpoint at the interval specified by Salesforce until the user authorizes the request or it times out.
- **Error Handling:** Must correctly handle and surface `authorization_pending`, `slow_down` (by increasing the polling interval), `expired_token`, and `access_denied`.
- **Ergonomics:** Must provide a callback or channel to easily display the `user_code` and `verification_uri` to the user before the polling begins.

## 🚫 Out of Scope
- Building an actual CLI application (this is just the library support).
- Storing or caching the resulting tokens (that is the caller's responsibility).
