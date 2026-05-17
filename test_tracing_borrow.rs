fn main() {
    let mut s = String::from("hello");
    let span = tracing::info_span!("test", field = s.as_str());
    let _g = span.enter();
    s.push_str(" world"); // If span borrows s, this will fail
}
