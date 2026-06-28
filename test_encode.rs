use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

fn main() {
    let external_id_value = "test test";
    let encoded_value = utf8_percent_encode(external_id_value, NON_ALPHANUMERIC);

    // Instead of format!, we can stream into a pre-allocated string
    let mut api_path = String::with_capacity(30 + external_id_value.len() * 3);
    api_path.push_str("sobjects/");
    api_path.push_str("Account");
    api_path.push('/');
    api_path.push_str("MyId");
    api_path.push('/');

    for chunk in encoded_value {
        api_path.push_str(chunk);
    }

    println!("{}", api_path);
}
