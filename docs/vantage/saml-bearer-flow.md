# 🔭 Vantage: Spec for SAML Bearer Flow

## Overview
A specification to implement the OAuth 2.0 SAML Bearer Assertion Flow for Salesforce authentication.

## 👤 User Story
As an Enterprise Integration Architect, I want to authenticate our internal systems with Salesforce using SAML Bearer assertions, so that I can reuse our existing corporate identity provider (IdP) integrations without distributing long-lived client secrets.

## The "So What?"
**What business problem does this solve?**
Many large enterprise organizations have strict compliance mandates that prohibit the use of client secrets or long-lived refresh tokens in middleware or integration pipelines. They mandate SAML for identity federation to mitigate the risk of secret sprawl. The SAML Bearer flow allows a system to generate a short-lived, digitally signed assertion representing an authenticated user, and exchange it for a Salesforce access token. By supporting this natively, we unlock usage for highly regulated enterprise customers who mandate SAML for all service-to-service authentication.

## 📈 Success Metrics
- **Success =** Ability to exchange a valid, pre-signed SAML 2.0 assertion at the Salesforce token endpoint for a valid access token in under 200ms.
- **Adoption:** >10% of new enterprise client instances use SAML authentication instead of legacy password or client-credential flows within 6 months.
- **Reliability:** 99.9% token acquisition success rate given a valid assertion.

## Gap Analysis
The library currently supports the Client Credentials, JWT Bearer, and legacy Username/Password flows. However, for organizations standardized entirely on SAML for enterprise identity, the absence of the SAML Bearer Flow means they cannot use the crate out-of-the-box. Developers are currently forced to hand-roll complex XML signatures and manage the OAuth exchange manually.

## ✅ Acceptance Criteria
- **Flow Support:** Must securely negotiate a Salesforce access token using a provided, valid, signed SAML assertion via the `/services/oauth2/token` endpoint.
- **Auto-Refresh:** Must automatically acquire a new access token when the current token expires, utilizing a new assertion if necessary.
- **Security:** Must fail securely and return clear domain errors if the SAML assertion is expired, incorrectly signed, or rejected by Salesforce.
- **Ergonomics:** The configuration should seamlessly integrate with the existing client building process, making it as simple as providing the pre-signed assertion and the token endpoint.

## 🚫 Out of Scope
- Building a full SAML Identity Provider (IdP) or parsing inbound SAML responses from SSO logins. This feature is strictly for the outbound SAML assertion token exchange.
- Generating or signing the raw XML SAML assertions. The calling application is expected to provide the pre-signed assertion to the library.
- Browser-based SAML web single sign-on (SSO) redirects.
