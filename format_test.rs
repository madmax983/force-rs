use std::fmt;

fn build_postman_url(name: &str, with_record_id: bool) -> serde_json::Value {
    let raw = if with_record_id {
        format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}/{{{{recordId}}}}", name)
    } else {
        format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}", name)
    };

    let mut path = vec![
        "services",
        "data",
        "v60.0",
        "sobjects",
        name,
    ];

    if with_record_id {
        path.push("{{recordId}}");
    }

    serde_json::json!({
        "raw": raw,
        "host": [
            "{{_endpoint}}"
        ],
        "path": path
    })
}

fn main() {
}
