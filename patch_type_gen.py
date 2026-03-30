def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    new_snake = '''    fn snake_case(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 2);
        let mut prev_char: Option<char> = None;

        for c in s.chars() {
            if c.is_ascii_uppercase() {
                if let Some(p) = prev_char {
                    if !p.is_ascii_uppercase() && p != '_' {
                        result.push('_');
                    }
                }
                result.push(c.to_ascii_lowercase());
            } else {
                result.push(c);
            }
            prev_char = Some(c);
        }'''

    flat_snake = '''    fn snake_case(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 2);
        let mut prev_char: Option<char> = None;

        for c in s.chars() {
            if !c.is_ascii_uppercase() {
                result.push(c);
                prev_char = Some(c);
                continue;
            }

            if let Some(p) = prev_char {
                if !p.is_ascii_uppercase() && p != '_' {
                    result.push('_');
                }
            }
            result.push(c.to_ascii_lowercase());
            prev_char = Some(c);
        }'''

    content = content.replace(new_snake, flat_snake)

    with open(filepath, 'w') as f:
        f.write(content)

process_file('crates/force/src/experimental/type_generator.rs')
