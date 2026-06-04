# 🔭 Vantage: Spec for OAuth 2.0 Device Flow

## User Story
As a CLI tool developer, I want to authenticate users via the OAuth 2.0 Device Authorization Grant, so that my users can securely log into Salesforce from headless environments or terminals without requiring a local web server for callbacks.

## The "So What?"
**What business problem does this solve?**
Many developers use force-rs to build automation scripts, CI/CD tools, and terminal applications. These environments lack a browser or the ability to easily handle localhost callbacks required by the standard Web Server flow. Implementing the Device Flow reduces friction, enabling secure, interactive authentication for headless operations.

## Metric Definition
- **Success =** Ability to initiate a device flow, retrieve a user code/verification URL, poll the token endpoint according to the specified interval, and successfully acquire an access token once the user authorizes the request.

## Gap Analysis
The force crate currently supports JWT Bearer, SAML Bearer, Client Credentials, and Username/Password authentication. However, it lacks a standard interactive flow suitable for CLI applications, forcing developers to implement complex, external web-based OAuth flows or rely on less secure methods like Username/Password (which is being deprecated).

## Acceptance Criteria
- Must support the standard OAuth 2.0 Device Authorization Grant specification.
- Must handle the initiation step to retrieve device_code, user_code, verification_uri, and interval.
- Must provide a mechanism to present the user_code and verification_uri to the user.
- Must implement the polling mechanism respecting the interval and handling authorization_pending and slow_down responses.

## Out of Scope
- OAuth 2.0 Web Server Flow (Auth Code Grant).
- Headless automated approval of the device code.
