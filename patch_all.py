import re

def process_error():
    filepath = 'crates/force/src/error.rs'
    with open(filepath, 'a') as f:
        f.write('''
/// Unwraps the `Result`, or panics with the given context and error message.
///
/// This provides the `unwrap_or_panic` method used in builder patterns (e.g. `SoqlQueryBuilder`)
/// to return non-panicking `Result` implementations while maintaining a panicking builder API.
pub fn unwrap_or_panic<T>(result: Result<T, crate::error::ForceError>, context: &str) -> T {
    match result {
        Ok(val) => val,
        Err(e) => panic!("Invalid input in {}: {}", context, e),
    }
}
''')

def process_soql():
    filepath = 'crates/force/src/api/soql.rs'
    with open(filepath, 'r') as f:
        content = f.read()

    content = content.replace("use crate::error::ForceError;", "use crate::error::{ForceError, unwrap_or_panic};")

    content = re.sub(r'\s+fn unwrap_or_panic<T>\(result: Result<T, ForceError>, context: &str\) -> T \{\s+result\.unwrap_or_else\(\|e\| panic!\("Invalid input in \{\}: \{\}", context, e\)\)\s+\}', '', content)

    replacements = [
        ("Self::unwrap_or_panic(", "unwrap_or_panic(")
    ]
    for old, new in replacements:
        content = content.replace(old, new)

    with open(filepath, 'w') as f:
        f.write(content)

def process_search():
    filepath = 'crates/force/src/api/rest/search.rs'
    with open(filepath, 'r') as f:
        content = f.read()

    content = content.replace("use crate::types::validator::validate_sobject_name;", "use crate::error::unwrap_or_panic;\nuse crate::types::validator::validate_sobject_name;")

    content = re.sub(r'\s+/// Helper to unwrap `try_` methods or panic with a clear message\.\s+fn unwrap_or_panic<T>\(result: Result<T, crate::error::ForceError>, context: &str\) -> T \{\s+result\.unwrap_or_else\(\|e\| panic!\("Invalid input in \{\}: \{\}", context, e\)\)\s+\}', '', content)

    replacements = [
        ("Self::unwrap_or_panic(", "unwrap_or_panic(")
    ]
    for old, new in replacements:
        content = content.replace(old, new)

    with open(filepath, 'w') as f:
        f.write(content)

process_error()
process_soql()
process_search()
