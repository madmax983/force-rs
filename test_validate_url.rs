fn main() {
    let result = force::types::validator::validate_url_path("http:/foo/bar");
    println!("{:?}", result);
    let result2 = force::types::validator::validate_url_path("file:///etc/passwd");
    println!("{:?}", result2);
}
