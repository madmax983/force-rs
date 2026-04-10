fn main() {
    let field = "A;";
    let mut balance = 0;
    let mut in_quote = None;
    let mut escaped = false;

    for c in field.chars() {
        if escaped {
            escaped = false;
            continue;
        }

        if c == '\\' {
            escaped = true;
            continue;
        }

        if let Some(quote_char) = in_quote {
            if c == quote_char {
                in_quote = None;
            }
        } else {
            match c {
                '\'' | '"' => in_quote = Some(c),
                '(' => balance += 1,
                ')' => {
                    balance -= 1;
                    if balance < 0 {
                        println!("unbalanced parentheses");
                        return;
                    }
                }
                _ if c.is_ascii_alphanumeric() => {}
                '_' | '.' | ' ' | '\t' | '\n' | '\r' | ',' | '=' | '!' | '<' | '>' | '-' | '+'
                | ':' | '%' | '&' | '|' | '^' | '*' | '$' => {}
                ';' | '{' | '}' | '[' | ']' => {
                    println!("invalid char outside quote");
                    return;
                }
                _ => {
                    println!("invalid char");
                    return;
                }
            }
        }
    }
}
