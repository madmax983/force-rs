# 🔭 Vantage: Spec for SAML Bearer Flow

## Overview
A specification to implement the OAuth 2.0 SAML Bearer Assertion Flow for Salesforce authentication.

## User Story
As a Security Architect or Enterprise Integrator, I want to authenticate to Salesforce using the SAML Bearer Flow, so that I can leverage my organization's existing enterprise identity infrastructure (like Okta or Azure AD) for secure, server-to-server integrations without storing plain client secrets or long-lived refresh tokens.

## The "So What?"
**What business problem does this solve?**
Many large enterprise organizations have strict compliance mandates that prohibit the use of client secrets or long-lived refresh tokens in middleware or integration pipelines. The SAML 2.0 Bearer Assertion flow allows a system to generate a short-lived, digitally signed assertion representing an authenticated user, and exchange it for a Salesforce access token. By supporting this flow natively in `force-rs`, we unlock usage for highly regulated enterprise customers who mandate SAML for all service-to-service authentication.

## Metric Definition
- **Success =** Ability to programmatically generate a valid SAML 2.0 assertion, sign it with an X.509 certificate, and exchange it at the Salesforce token endpoint for a valid access token in under 200ms.

## Gap Analysis
The `force-rs` crate currently supports the Client Credentials, JWT Bearer, and legacy Username/Password flows. However, for organizations standardized entirely on SAML for enterprise identity, the absence of the SAML Bearer Flow means they cannot use the crate out-of-the-box. Developers are currently forced to hand-roll complex XML signatures and manage the OAuth exchange manually before passing the token to `force-rs`.

## Acceptance Criteria
- Must implement a new authentication provider that integrates with the core authentication flow.
- Must support generating and formatting a valid XML SAML 2.0 assertion containing the required elements (Issuer, Subject, Audience, Expiration).
- Must digitally sign the assertion using standard RSA-SHA256 with a provided private key.
- Must successfully exchange the signed assertion at the `/services/oauth2/token` endpoint using `grant_type=urn:ietf:params:oauth:grant-type:saml2-bearer`.
- Must seamlessly integrate with the existing client building process.

## Out of Scope
- Building a full SAML Identity Provider (IdP) or parsing inbound SAML responses from SSO logins. This feature is strictly for the outbound SAML assertion token exchange.
- Automatic rotation or fetching of X.509 certificates from a remote Key Management Service (providing the key bytes is the caller's responsibility).
