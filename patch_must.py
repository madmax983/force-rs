import re

with open("crates/force/src/http/tests.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'let response = result\.unwrap\(\);',
    'let response = result.must();',
    content
)

with open("crates/force/src/http/tests.rs", "w") as f:
    f.write(content)

with open("crates/force/src/api/bulk/csv.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'serialize_to_csv\(&records, &mut output\)\.unwrap\(\);',
    'serialize_to_csv(&records, &mut output).must();',
    content
)

content = re.sub(
    r'let records: Vec<TestRecord> = deserialize_from_csv\(csv_data\.as_bytes\(\)\)\.unwrap\(\);',
    'let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).must();',
    content
)

content = re.sub(
    r'process_csv_batches\(csv_data\.as_bytes\(\), 2, \|batch: Vec<TestRecord>\| \{\n            batch_count \+= 1;\n            total_records \+= batch\.len\(\);\n            assert!\(batch\.len\(\) <= 2\);\n            Ok\(\(\)\)\n        \}\)\.unwrap\(\);',
    r'process_csv_batches(csv_data.as_bytes(), 2, |batch: Vec<TestRecord>| {\n            batch_count += 1;\n            total_records += batch.len();\n            assert!(batch.len() <= 2);\n            Ok(())\n        }).must();',
    content
)

content = re.sub(
    r'process_csv_batches\(csv_data\.as_bytes\(\), 100, \|batch: Vec<TestRecord>\| \{\n            batch_count \+= 1;\n            assert_eq!\(batch\.len\(\), 2\);\n            Ok\(\(\)\)\n        \}\)\.unwrap\(\);',
    r'process_csv_batches(csv_data.as_bytes(), 100, |batch: Vec<TestRecord>| {\n            batch_count += 1;\n            assert_eq!(batch.len(), 2);\n            Ok(())\n        }).must();',
    content
)

with open("crates/force/src/api/bulk/csv.rs", "w") as f:
    f.write(content)
