import re

with open('crates/force/src/client/mod.rs', 'r') as f:
    content = f.read()

# Replace test_builder_creates_noauth_state
new_test1 = """    #[test]
    fn test_builder_creates_noauth_state() {
        let builder_instance = builder();
        // Since the return type is ForceClientBuilder<NoAuth>, we just need
        // to assert that it is the correct default configuration or state
        // To kill the Default::default mutant, we could check something specific
        // But since ForceClientBuilder doesn't have public accessors for config early on,
        // we can check if it creates the correct type implicitly.
        // Actually the mutant is `ForceClientBuilder::from(Default::default())`.
        // We will build it and see if the configuration matches our expectation.
        let config = builder_instance.config();
        assert_eq!(config.api_version, "v60.0");
        assert_eq!(config.timeout.as_secs(), 30);
    }"""

content = re.sub(r'    #\[test\]\s+fn test_builder_creates_noauth_state\(\) \{\s+let _builder = builder\(\);\s+// Check that the builder is initialized properly\.\s+\}', new_test1, content)

# Add missing tests for bulk and composite methods
new_tests2 = """
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
}"""

content = re.sub(r'\}\s*$', new_tests2, content)

with open('crates/force/src/client/mod.rs', 'w') as f:
    f.write(content)
