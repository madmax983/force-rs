import re

with open("crates/force/src/http/tests.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+let response = result\.must\(\);',
    'let response = result.expect("Expected successful request, but got error");',
    content
)

content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+assert_eq!\(refresh_count\.load\(Ordering::SeqCst\), 1\);',
    'result.expect("Expected successful token refresh");\n        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);',
    content
)

content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+// Should have waited ~10ms \+ ~20ms = ~30ms for backoff\s+assert!\(elapsed\.as_millis\(\) >= 25\);',
    'result.expect("Expected successful retry");\n        // Should have waited ~10ms + ~20ms = ~30ms for backoff\n        assert!(elapsed.as_millis() >= 25);',
    content
)

content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+\}',
    'result.expect("Expected successful retry with explicit policy");\n    }',
    content
)

content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+assert_eq!\(retries\.load\(Ordering::SeqCst\), 1\);',
    'result.expect("Expected successful request");\n        assert_eq!(retries.load(Ordering::SeqCst), 1);',
    content
)

content = re.sub(
    r'let result: Result<TestResponse, ForceError> = executor\n            \.execute_json\(request, &token, \|\| async \{ panic!\("Should not refresh"\) \}\)\n            \.await;\n\n        // Assert\n        assert!\(result\.is_ok\(\)\);\n        let response = result\.must\(\);',
    'let result: Result<TestResponse, ForceError> = executor\n            .execute_json(request, &token, || async { panic!("Should not refresh") })\n            .await;\n\n        // Assert\n        let response = result.expect("Expected successful request, but got error");',
    content
)


content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+// Should have waited at least 50ms \(timeout\) \+ 10ms \(backoff\)\s+assert!\(elapsed\.as_millis\(\) >= 60\);',
    'result.expect("Expected successful request");\n        // Should have waited at least 50ms (timeout) + 10ms (backoff)\n        assert!(elapsed.as_millis() >= 60);',
    content
)


content = re.sub(
    r'assert!\(result\.is_ok\(\)\);\s+let response = result\.must\(\);',
    'let response = result.expect("Expected successful mutation retry");',
    content
)


with open("crates/force/src/http/tests.rs", "w") as f:
    f.write(content)
