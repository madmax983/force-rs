use force::types::validator::validate_url_path;

fn main() {
    println!("{:?}", validate_url_path("http:/foo/bar"));
    println!("{:?}", validate_url_path("file:///etc/passwd"));
    println!("{:?}", validate_url_path("ftp://evil.com/payload"));
    println!("{:?}", validate_url_path("javascript:alert(1)"));
}
