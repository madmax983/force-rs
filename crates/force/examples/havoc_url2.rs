use force::types::validator::validate_url_path;
use url::Url;

fn main() {
    let base = Url::parse("http://localhost").unwrap();
    let result = base.join("http:/foo/bar");
    println!("http:/foo/bar joins to: {:?}", result);
    let result2 = base.join("javascript:alert(1)");
    println!("javascript:alert(1) joins to: {:?}", result2);
    let result3 = base.join("https:evil.com");
    println!("https:evil.com joins to: {:?}", result3);
}
