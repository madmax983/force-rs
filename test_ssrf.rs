fn main() {
    let bad_url = "javascript:alert(1)";
    let url_validation = force::types::validator::validate_url_path(bad_url);
    println!("Validation for {}: {:?}", bad_url, url_validation);

    let base = url::Url::parse("http://localhost").unwrap();
    println!("Base parsed: {}", base);
    let joined = base.join(bad_url).unwrap();
    println!("Joined: {}", joined);
}
