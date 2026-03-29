import re
filepath = 'crates/force/src/experimental/soql_mass_op.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace("assert_ne!(stats.records_processed, Default::default());", "assert_ne!(stats.records_processed, 0);")
content = content.replace("assert_ne!(stats.ops_succeeded, Default::default());", "assert_ne!(stats.ops_succeeded, 0);")

with open(filepath, 'w') as f:
    f.write(content)
