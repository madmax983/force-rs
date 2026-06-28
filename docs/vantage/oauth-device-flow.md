# 🔭 Vantage: Spec for OAuth 2.0 Device Flow

## Overview
A specification to implement the OAuth 2.0 Device Flow for Salesforce authentication.

## 👤 User Story
As a Developer Tooling Engineer, I want to authenticate our CLI tools with Salesforce using the OAuth 2.0 Device Flow, so that users can log in interactively from headless environments or terminals without requiring a local web server.

## The "So What?" (Business Problem)
Many developers and administrators interact with Salesforce through CLI tools, CI/CD pipelines, or remote servers that lack a GUI browser. The standard web server OAuth flow requires standing up a local web server to receive the callback, which is often blocked by corporate firewalls, containerized environments, or remote SSH sessions. The Device Flow solves this by providing authorization data that the user can enter on a separate device (like their phone or laptop browser). Supporting this unlocks native Rust CLI development for Salesforce, a growing ecosystem.

## 📈 Success Metric
- **Success =** Ability to initiate a device flow and successfully poll the token service until an access token is issued or the flow times out.
- **Adoption:** Used by at least one internal CLI tool within 3 months.
- **Reliability:** 99.9% success rate for completed flows with appropriate timeout handling.

## Gap Analysis
The library currently supports Client Credentials, JWT Bearer, and legacy Username/Password flows. However, all of these are machine-to-machine or headless flows that require pre-provisioned secrets (keys, passwords, or client secrets). There is currently no support for interactive human-in-the-loop authentication. Developers building CLI tools are forced to either use insecure passwords or implement the Device Flow polling mechanism manually.

## ✅ Acceptance Criteria
- **Flow Initiation:** Must securely initiate the interactive flow with the Salesforce authentication service to retrieve the required authorization data.
- **Polling:** Must implement a polling mechanism to check the Salesforce token service at the specified interval until the user authorizes the request, it expires, or is denied.
- **Security:** Must securely handle rate limiting and pending responses from the token service without failing the flow prematurely.
- **Ergonomics:** Must provide a mechanism to pass the required interactive authorization data to the calling application so it can be displayed to the user.

## 🚫 Out of Scope
- Building the actual CLI UI/terminal prompt to display the authorization data.
- Storing or caching the resulting tokens on disk.
- Browser-based Web Server OAuth Flow.