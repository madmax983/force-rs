use reqwest::{Request, Method, Url};
fn test(mut req: Request) {
    let method = req.method().as_str();
    let path = req.url().path();

    // Some context
    let ctx = (method, path);

    req.headers_mut().insert("auth", "token".parse().unwrap());

    println!("{:?}", ctx);
}
fn main() {}
