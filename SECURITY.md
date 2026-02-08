# Security Policy

## Reporting Security Vulnerabilities

We take the security of `force-rs` seriously. If you discover a security vulnerability, please follow these steps:

### Preferred Method: GitHub Security Advisories

1. Go to https://github.com/madmax983/force-rs/security/advisories
2. Click "Report a vulnerability"
3. Provide a detailed description of the vulnerability including:
   - Type of vulnerability (e.g., authentication bypass, credential exposure)
   - Steps to reproduce
   - Affected versions
   - Potential impact
   - Any suggested fixes

### Alternative Contact

If you prefer not to use GitHub Security Advisories, you can reach out via:
- Email: security@[project-domain] (TODO: Update with actual contact)

**Please do not:**
- Open a public GitHub issue for security vulnerabilities
- Disclose the vulnerability publicly before we've had a chance to address it

## Response Timeline

- **Initial Response**: Within 48 hours of report
- **Status Update**: Within 7 days with severity assessment
- **Fix Timeline**: Critical issues within 30 days; others based on severity

## Security Considerations for Users

### Credential Management

`force-rs` uses the `secrecy` crate to protect sensitive credentials in memory:
- OAuth tokens are wrapped in `Secret<String>` and never logged
- Passwords and client secrets are similarly protected
- Credentials are cleared from memory when dropped

**Best Practices:**
- Never hardcode credentials in source code
- Use environment variables or secure credential stores
- Enable audit logging in production Salesforce orgs
- Rotate OAuth client secrets regularly
- Use JWT Bearer Flow in production (not username-password)

### TLS/HTTPS

- All API communication uses HTTPS via `rustls` (no OpenSSL dependency)
- Certificate validation is always enabled
- Minimum TLS version: TLS 1.2

### Dependency Security

We use `cargo-deny` to audit dependencies for:
- Known security vulnerabilities (RUSTSEC advisories)
- License compliance
- Supply chain security

Run `cargo deny check` to verify our dependency tree.

### Rate Limiting & DDoS Protection

- Salesforce enforces API rate limits (see org limits API)
- Respect 429 responses and implement exponential backoff
- Bulk API 2.0 is designed for high-volume data operations

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Security Features

### Authentication Flows
- ✅ OAuth 2.0 Web Server Flow (authorization code)
- ✅ OAuth 2.0 JWT Bearer Flow (service accounts)
- ✅ Username-Password Flow (development only)
- ✅ Token refresh with automatic retry

### API Security
- ✅ HTTPS-only communication
- ✅ Bearer token authentication
- ✅ Token expiration handling
- ✅ Audit trail support (API logging)

### Data Protection
- ✅ In-memory credential protection (`secrecy` crate)
- ✅ No credential logging
- ✅ Sensitive data cleared on drop
- ✅ Type-safe API preventing common mistakes

## Vulnerability Disclosure Policy

When we receive a security report:

1. **Confirmation**: We confirm receipt and validate the issue
2. **Investigation**: We assess severity and exploitability
3. **Fix Development**: We develop and test a fix
4. **Coordinated Disclosure**: We work with reporter on disclosure timeline
5. **Release**: We publish a security advisory and patched version
6. **Credit**: We credit the reporter (unless they prefer anonymity)

## Security Advisories

All security advisories will be published at:
https://github.com/madmax983/force-rs/security/advisories

## Related Security Resources

- [Salesforce Security Guide](https://help.salesforce.com/s/articleView?id=sf.security_overview.htm)
- [OAuth 2.0 Best Practices](https://datatracker.ietf.org/doc/html/draft-ietf-oauth-security-topics)
- [RUSTSEC Advisory Database](https://rustsec.org/)

## Acknowledgments

We appreciate the security research community's efforts to keep open source software safe. Security researchers who responsibly disclose vulnerabilities will be acknowledged in our security advisories and release notes (if desired).
