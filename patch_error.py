import re

with open('crates/force/src/error.rs', 'r') as f:
    content = f.read()

# Completely remove all instances of `unwrap_or_panic` at the end of the file.
content = re.sub(r'/// Unwraps the `Result`[\s\S]+?\}\s*$', '', content)
content = re.sub(r'pub fn unwrap_or_panic[\s\S]+?\}\s*$', '', content)

# Now define it properly right before `mod tests`
proper_def = '''
/// Unwraps the `Result`, or panics with the given context and error message.
///
/// This provides the `unwrap_or_panic` method used in builder patterns (e.g. `SoqlQueryBuilder`)
/// to return non-panicking `Result` implementations while maintaining a panicking builder API.
///
/// # Panics
///
/// Panics if the result is `Err`.
pub fn unwrap_or_panic<T>(
    result: std::result::Result<T, crate::error::ForceError>,
    context: &str,
) -> T {
    match result {
        Ok(val) => val,
        Err(e) => panic!("Invalid input in {}: {}", context, e),
    }
}
'''

content = content.replace('#[cfg(test)]\nmod tests {', proper_def + '\n#[cfg(test)]\nmod tests {')

with open('crates/force/src/error.rs', 'w') as f:
    f.write(content)
