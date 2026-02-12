**2024-05-23 - [Havoc: Access Token Overflow Panic]**
**Threat:** A malicious or corrupted Salesforce OAuth response with an extremely large `expires_in` value combined with a large `issued_at` timestamp causes `chrono::Duration` addition to overflow, leading to a panic and Denial of Service.
**Defense:** Replaced unchecked addition with `checked_add_signed` in `AccessToken::from_response`. If the expiration time overflows `DateTime<Utc>::MAX`, the token is treated as having no expiration.
