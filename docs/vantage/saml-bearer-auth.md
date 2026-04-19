# SAML Bearer Authentication Flow

👤 **User Story:** "As an Enterprise Integration Architect, I want to authenticate our internal systems with Salesforce using SAML Bearer assertions, so that I can reuse our existing corporate identity provider (IdP) integrations without distributing long-lived client secrets."

**The "So What?" (Business Problem):**
Many large organizations mandate SAML for identity federation. They are actively trying to deprecate standalone client credentials or service accounts due to the risk of secret sprawl. The SAML Bearer flow allows them to issue temporary assertions from their internal IdP to authorize server-to-server integrations with Salesforce, maintaining central compliance.

✅ **Acceptance Criteria:**
- **Flow Support:** Must securely negotiate a Salesforce access token using a provided, valid, signed SAML assertion.
- **Auto-Refresh:** Must automatically acquire a new access token when the current token expires, utilizing a new assertion if necessary (or alerting the caller that a new assertion is needed).
- **Security:** Must fail securely (and clearly) if the SAML assertion is expired, incorrectly signed, or rejected by Salesforce.
- **Ergonomics:** The configuration should be as simple as providing the assertion and the token endpoint.

📈 **Success Metrics:**
- **Adoption:** >10% of new enterprise client instances use SAML auth instead of legacy password or client-credential flows within 6 months.
- **Reliability:** 99.9% token acquisition success rate given a valid assertion.

🚫 **Out of Scope:**
- Building a full SAML IdP (Identity Provider). We are the relying party / client in this context.
- Generating or signing the raw XML SAML assertions. The calling application is expected to provide the pre-signed assertion to the library.
- Browser-based SAML web single sign-on (SSO) redirects.
