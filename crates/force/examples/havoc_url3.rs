use force::types::validator::validate_url_path;

fn main() {
    let bad_url = "javascript:alert(1)";
    let url_validation = validate_url_path(bad_url);
    println!("Validation for {}: {:?}", bad_url, url_validation);

    let bad_url_2 = "https:evil.com";
    let url_validation_2 = validate_url_path(bad_url_2);
    println!("Validation for {}: {:?}", bad_url_2, url_validation_2);

    let bad_url_3 = "file:///etc/passwd";
    let url_validation_3 = validate_url_path(bad_url_3);
    println!("Validation for {}: {:?}", bad_url_3, url_validation_3);
}
