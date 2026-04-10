fn main() {
    let text = "ü?";
    let first_special = text.find(['?']);
    println!("{:?}", first_special);
    if let Some(idx) = first_special {
        println!("{}", &text[..idx]);
        for c in text[idx..].chars() {
            println!("{}", c);
        }
    }
}
