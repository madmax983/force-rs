use reqwest::{Request, Method, Url};

fn test_fn(request: &Request) {
    let method_str = request.method().as_str();
    let path_str = request.url().path();

    let request_span = tracing::info_span!(
        "force_http_request",
        http.method = method_str,
        http.path = path_str,
    );
}
fn main() {}
