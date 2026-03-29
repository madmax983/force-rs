import re

filepath = 'crates/force/src/experimental/soql_mass_op.rs'
with open(filepath, 'r') as f:
    content = f.read()

target = '        assert_eq!(stats.records_processed, 2);\n        assert_eq!(stats.ops_succeeded, 2);\n        assert_eq!(stats.ops_failed, 0);'
replacement = '        assert_eq!(stats.records_processed, 2);\n        assert_eq!(stats.ops_succeeded, 2);\n        assert_eq!(stats.ops_failed, 0);\n        assert_ne!(stats.records_processed, 0);\n        assert_ne!(stats.ops_succeeded, 0);'
content = content.replace(target, replacement)

target2 = '        assert_eq!(stats.records_processed, 1);\n        assert_eq!(stats.ops_succeeded, 1);\n        assert_eq!(stats.ops_failed, 0);'
replacement2 = '        assert_eq!(stats.records_processed, 1);\n        assert_eq!(stats.ops_succeeded, 1);\n        assert_eq!(stats.ops_failed, 0);\n        assert_ne!(stats.records_processed, 0);\n        assert_ne!(stats.ops_succeeded, 0);'
content = content.replace(target2, replacement2)

with open(filepath, 'w') as f:
    f.write(content)
