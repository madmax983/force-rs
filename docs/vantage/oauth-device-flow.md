# 🔭 Vantage: Spec for OAuth 2.0 Device Flow

## Overview
A specification to implement the OAuth 2.0 Device Flow for Salesforce authentication.

## 👤 User Story
As a CLI Developer, I want to authenticate users using the OAuth Device Flow, so that users can log in on headless environments or devices with limited input capabilities.

## The "So What?"
**What business problem does this solve?**
Many enterprise developers build CLI tools or background workers that run in headless server environments, CI/CD pipelines, or edge devices where a browser cannot be launched. The OAuth Device Flow allows these tools to display a simple user code and URL, prompting the user to authorize on their own workstation. Without this, developers must rely on legacy password flows or complex server setups. By natively supporting this, we enable seamless CLI and daemon integration without compromising security.

## 📈 Success Metrics
- **Success =** Ability to initiate the device flow, poll for user authorization, and successfully retrieve an access token.
- **Adoption:** Used by >50% of CLI tools built on `force-rs`.
- **Reliability:** 99.9% token acquisition success rate after successful user authorization.

## Gap Analysis
The library currently supports Client Credentials, JWT Bearer, and Username/Password, but lacks support for headless user-interactive authentication. Developers building CLI tools are forced to build custom HTTP polling logic for the device flow, which is error-prone and tedious.

## ✅ Acceptance Criteria
- **Initiation:** Must be able to request a device code, user code, and verification URL from the Salesforce token endpoint.
- **Polling:** Must implement an async polling mechanism that respects standard device flow polling responses and intervals correctly.
- **Token Retrieval:** Must securely retrieve the access token once the user completes authorization.
- **Ergonomics:** Must provide a callback or streaming interface to display the user code and URL to the end-user.

## 🚫 Out of Scope
- Implementing a local web server for the traditional Web Server Flow.
- Refreshing tokens (unless requested as a separate feature, though typically handled via the refresh token flow).