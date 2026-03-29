import re

filepath = 'crates/force/src/experimental/soql_mass_op.rs'
with open(filepath, 'r') as f:
    content = f.read()

target = '            .respond_with(ResponseTemplate::new(200).set_body_json(json!({\n                "totalSize": 1,\n                "done": true,\n                "records": [ { "attributes": { "type": "Account", "url": "/services/data/v60.0/sobjects/Account/001A" }, "Id": "001A" } ]\n            })))'

replacement = '            .respond_with(ResponseTemplate::new(200).set_body_json(json!({\n                "totalSize": 1,\n                "done": true,\n                "records": [ { "attributes": { "type": "Account", "url": "/services/data/v60.0/sobjects/Account/001000000000001AAA" }, "Id": "001000000000001AAA" } ]\n            })))'

if target in content:
    updated_content = content.replace(target, replacement)
    with open(filepath, 'w') as f:
        f.write(updated_content)
    print(f"Successfully fixed ID length in {filepath}")
else:
    print(f"Target string not found in {filepath}")
