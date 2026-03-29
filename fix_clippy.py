import re

filepath = 'crates/force/src/api/tooling/run_tests.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace('assert_eq!(result.all_passed(), true);', 'assert!(result.all_passed());')
content = content.replace('assert_eq!(result.all_passed(), false);', 'assert!(!result.all_passed());')

with open(filepath, 'w') as f:
    f.write(content)

filepath_builder = 'crates/force/src/client/builder.rs'
with open(filepath_builder, 'r') as f:
    content = f.read()

content = content.replace('client.dc_session.unwrap()', 'client.dc_session.must()')

with open(filepath_builder, 'w') as f:
    f.write(content)
