with open('crates/force/src/api/composite/graph.rs', 'r') as f:
    content = f.read()

new_content = content.replace('result\n                .unwrap_err()\n                .to_string()', 'if let Err(e) = result {\n            e.to_string()\n        } else {\n            panic!("Expected Err")\n        }')
new_content = new_content.replace('assert!(result.is_err());\n        assert!(\n            if', 'assert!(\n            if')

with open('crates/force/src/api/composite/graph.rs', 'w') as f:
    f.write(new_content)
