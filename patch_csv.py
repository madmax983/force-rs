import re

with open("crates/force/src/api/bulk/csv.rs", "r") as f:
    content = f.read()

# Replace test_serialize_empty_records
content = re.sub(
    r'let result = serialize_to_csv\(&records, &mut output\);\s+assert!\(result\.is_ok\(\)\);',
    'serialize_to_csv(&records, &mut output).expect("Failed to serialize empty records to CSV");',
    content
)

# Replace test_deserialize_empty_csv
content = re.sub(
    r'let result: Result<Vec<TestRecord>> = deserialize_from_csv\(csv_data\.as_bytes\(\)\);\s+assert!\(result\.is_ok\(\)\);\s+let records = result\.must\(\);',
    'let records: Vec<TestRecord> = deserialize_from_csv(csv_data.as_bytes()).expect("Failed to deserialize empty CSV data");',
    content
)

# Replace test_process_batches_small
content = re.sub(
    r'let result = process_csv_batches\(csv_data\.as_bytes\(\), 2, \|batch: Vec<TestRecord>\| \{\s+batch_count \+= 1;\s+total_records \+= batch\.len\(\);\s+assert!\(batch\.len\(\) <= 2\);\s+Ok\(\(\)\)\s+\}\);\s+assert!\(result\.is_ok\(\)\);',
    r'process_csv_batches(csv_data.as_bytes(), 2, |batch: Vec<TestRecord>| {\n            batch_count += 1;\n            total_records += batch.len();\n            assert!(batch.len() <= 2);\n            Ok(())\n        }).expect("Failed to process batches for small dataset");',
    content
)

# Replace test_process_batches_large
content = re.sub(
    r'let result = process_csv_batches\(csv_data\.as_bytes\(\), 100, \|batch: Vec<TestRecord>\| \{\s+batch_count \+= 1;\s+assert_eq!\(batch\.len\(\), 2\);\s+Ok\(\(\)\)\s+\}\);\s+assert!\(result\.is_ok\(\)\);',
    r'process_csv_batches(csv_data.as_bytes(), 100, |batch: Vec<TestRecord>| {\n            batch_count += 1;\n            assert_eq!(batch.len(), 2);\n            Ok(())\n        }).expect("Failed to process batches for large dataset");',
    content
)

# Replace test_serialize_large_dataset
content = re.sub(
    r'let result = serialize_to_csv\(&records, &mut output\);\s+assert!\(result\.is_ok\(\)\);',
    r'serialize_to_csv(&records, &mut output).expect("Failed to serialize large dataset");',
    content
)

with open("crates/force/src/api/bulk/csv.rs", "w") as f:
    f.write(content)
