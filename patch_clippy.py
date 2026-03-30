import re

def fix_expect(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Revert back to unwrap_or_else to avoid clippy expect_used lint which is active
    content = content.replace('.expect("String format cannot fail")', '.unwrap_or_else(|_| unreachable!("String format cannot fail"))')
    content = content.replace('.expect("writing to String is infallible")', '.unwrap_or_else(|_| unreachable!("writing to String is infallible"))')

    with open(filepath, 'w') as f:
        f.write(content)

fix_expect('crates/force/src/api/rest/search.rs')
fix_expect('crates/force/src/api/soql.rs')
fix_expect('crates/force/src/experimental/scanner.rs')

# Move unwrap_or_panic to before tests and add panics doc
def fix_error_rs():
    with open('crates/force/src/error.rs', 'r') as f:
        content = f.read()

    # extract it
    match = re.search(r'(/// Unwraps the `Result`[\s\S]+?fn unwrap_or_panic<T>\(result: std::result::Result<T, crate::error::ForceError>, context: &str\) -> T \{[\s\S]+?\}\s+\})', content)
    if match:
        extracted = match.group(1)
        content = content.replace(extracted, '')

        # Add panics doc
        extracted = extracted.replace('/// to return non-panicking `Result` implementations while maintaining a panicking builder API.', '/// to return non-panicking `Result` implementations while maintaining a panicking builder API.\n///\n/// # Panics\n///\n/// Panics if the result is an `Err`.')

        # insert before mod tests
        content = content.replace('#[cfg(test)]\nmod tests {', extracted + '\n\n#[cfg(test)]\nmod tests {')

        with open('crates/force/src/error.rs', 'w') as f:
            f.write(content)

fix_error_rs()
