import re

with open("crates/force/src/http/tests.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'let response = result\.expect\("Expected successful request, but got error"\);',
    'let response = result.unwrap();',
    content
)

content = re.sub(
    r'result\.expect\("Expected successful token refresh"\);\n        assert_eq!\(refresh_count\.load\(Ordering::SeqCst\), 1\);',
    'assert!(result.is_ok());\n        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);',
    content
)

content = re.sub(
    r'result\.expect\("Expected successful retry"\);\n        // Should have waited ~10ms \+ ~20ms = ~30ms for backoff\n        assert!\(elapsed\.as_millis\(\) >= 25\);',
    'assert!(result.is_ok());\n        // Should have waited ~10ms + ~20ms = ~30ms for backoff\n        assert!(elapsed.as_millis() >= 25);',
    content
)

content = re.sub(
    r'result\.expect\("Expected successful retry with explicit policy"\);\n    \}',
    'assert!(result.is_ok());\n    }',
    content
)

content = re.sub(
    r'result\.expect\("Expected successful request"\);\n        assert_eq!\(retries\.load\(Ordering::SeqCst\), 1\);',
    'assert!(result.is_ok());\n        assert_eq!(retries.load(Ordering::SeqCst), 1);',
    content
)

content = re.sub(
    r'let result: Result<TestResponse, ForceError> = executor\n            \.execute_json\(request, &token, \|\| async \{ panic!\("Should not refresh"\) \}\)\n            \.await;\n\n        // Assert\n        let response = result\.unwrap\(\);',
    'let result: Result<TestResponse, ForceError> = executor\n            .execute_json(request, &token, || async { panic!("Should not refresh") })\n            .await;\n\n        // Assert\n        assert!(result.is_ok());\n        let response = result.unwrap();',
    content
)

content = re.sub(
    r'result\.expect\("Expected successful request"\);\n        // Should have waited at least 50ms \(timeout\) \+ 10ms \(backoff\)\n        assert!\(elapsed\.as_millis\(\) >= 60\);',
    'assert!(result.is_ok());\n        // Should have waited at least 50ms (timeout) + 10ms (backoff)\n        assert!(elapsed.as_millis() >= 60);',
    content
)


with open("crates/force/src/http/tests.rs", "w") as f:
    f.write(content)

with open("crates/force/src/api/bulk/csv.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'serialize_to_csv\(&records, &mut output\)\.expect\("Failed to serialize empty records to CSV"\);',
    'serialize_to_csv(&records, &mut output).unwrap();',
    content
)

content = re.sub(
    r'let records: Vec<TestRecord> = deserialize_from_csv\(csv_data\.as_bytes\(\)\)\.expect\("Failed to deserialize empty CSV data"\);',
    'let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).unwrap();',
    content
)

content = re.sub(
    r'process_csv_batches\(csv_data\.as_bytes\(\), 2, \|batch: Vec<TestRecord>\| \{\n            batch_count \+= 1;\n            total_records \+= batch\.len\(\);\n            assert!\(batch\.len\(\) <= 2\);\n            Ok\(\(\)\)\n        \}\)\.expect\("Failed to process batches for small dataset"\);',
    r'process_csv_batches(csv_data.as_bytes(), 2, |batch: Vec<TestRecord>| {\n            batch_count += 1;\n            total_records += batch.len();\n            assert!(batch.len() <= 2);\n            Ok(())\n        }).unwrap();',
    content
)

content = re.sub(
    r'process_csv_batches\(csv_data\.as_bytes\(\), 100, \|batch: Vec<TestRecord>\| \{\n            batch_count \+= 1;\n            assert_eq!\(batch\.len\(\), 2\);\n            Ok\(\(\)\)\n        \}\)\.expect\("Failed to process batches for large dataset"\);',
    r'process_csv_batches(csv_data.as_bytes(), 100, |batch: Vec<TestRecord>| {\n            batch_count += 1;\n            assert_eq!(batch.len(), 2);\n            Ok(())\n        }).unwrap();',
    content
)

with open("crates/force/src/api/bulk/csv.rs", "w") as f:
    f.write(content)
