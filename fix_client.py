import re

with open('crates/force/src/client/mod.rs', 'r') as f:
    content = f.read()

new_tests = """
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockAuthenticator;
    use crate::test_support::Must;

    #[test]
    fn test_builder_creates_noauth_state() {
        let builder_instance = builder();
        // Just verify we can call a method on it to ensure it's not a dummy Default
        let _ = builder_instance.config(Default::default());
    }

    #[tokio::test]
    async fn test_force_client_clone() {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();

        let cloned_client = client.clone();

        // Assert that the cloned client points to the same underlying Arc
        assert!(Arc::ptr_eq(client.inner(), cloned_client.inner()));
    }

    #[cfg(feature = "bulk")]
    #[tokio::test]
    async fn test_bulk_handler_points_to_same_inner() {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();
        let bulk_handler = client.bulk();
        assert!(Arc::ptr_eq(client.inner(), bulk_handler.inner()));
    }

    #[cfg(feature = "composite")]
    #[tokio::test]
    async fn test_composite_handler_points_to_same_inner() {
        let auth = MockAuthenticator::new("test_token", "https://test.salesforce.com");
        let client = builder().authenticate(auth).build().await.must();
        let composite_handler = client.composite();
        assert!(Arc::ptr_eq(client.inner(), &composite_handler.inner));
    }
}
"""

content = re.sub(r'#\[cfg\(test\)\]\s*mod tests \{.*\}\s*$', new_tests.strip() + '\n', content, flags=re.DOTALL)

with open('crates/force/src/client/mod.rs', 'w') as f:
    f.write(content)
