import re

filepath = 'crates/force/src/api/tooling/run_tests.rs'
with open(filepath, 'r') as f:
    content = f.read()

# Replace the assertion in test_all_passed_with_failures
target = 'assert!(!result.all_passed());'
replacement = 'assert_eq!(result.all_passed(), false);'

if target in content:
    updated_content = content.replace(target, replacement)

    # Also add a check for the true case explicitly
    target2 = 'assert!(result.all_passed());'
    replacement2 = 'assert_eq!(result.all_passed(), true);'
    updated_content = updated_content.replace(target2, replacement2)

    with open(filepath, 'w') as f:
        f.write(updated_content)
    print(f"Successfully fixed assertions in {filepath}")
else:
    print(f"Target string not found in {filepath}")
