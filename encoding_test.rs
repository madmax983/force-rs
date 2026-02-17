use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

fn main() {
    const ENCODE_SET: percent_encoding::AsciiSet = NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');

    let input = "ACME-001";
    let encoded = utf8_percent_encode(input, &ENCODE_SET).to_string();
    println!("'{}' encoded: '{}'", input, encoded);
}
