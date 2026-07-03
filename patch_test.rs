fn main() {
    let name = String::from("Account");
    let label_str = String::from("");
    let name_ref = &name;

    let label = if label_str.is_empty() {
        name_ref
    } else {
        &label_str
    };

    println!("name: {}, label: {}", name_ref, label);
}
