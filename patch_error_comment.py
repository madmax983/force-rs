import re

with open("crates/force/src/http/error.rs", "r") as f:
    content = f.read()

# Fix doc comment placement properly this time
# The previous patch missed because of line numbers/whitespace
old_text = """/// Converts an HTTP error response into a `ForceError` using Salesforce-aware parsing.
///
/// If the response body is empty or unreadable, falls back to `fallback_message`.

/// Reads an HTTP response body up to a maximum size limit to prevent memory exhaustion DoS."""

new_text = """/// Reads an HTTP response body up to a maximum size limit to prevent memory exhaustion DoS."""

if old_text in content:
    content = content.replace(old_text, new_text)

    # Now we need to put the first comment block back in front of `pub async fn response_to_force_error`
    content = content.replace("pub async fn response_to_force_error(",
"""/// Converts an HTTP error response into a `ForceError` using Salesforce-aware parsing.
///
/// If the response body is empty or unreadable, falls back to `fallback_message`.
pub async fn response_to_force_error(""")

    with open("crates/force/src/http/error.rs", "w") as f:
        f.write(content)
    print("Fixed error.rs")
else:
    print("Could not find old_text")
