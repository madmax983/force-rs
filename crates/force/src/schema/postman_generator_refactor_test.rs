fn build_url(name: &str, record_id: bool) -> String {
    let raw = if record_id {
        format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}/{{{{recordId}}}}", name)
    } else {
        format!("{{{{_endpoint}}}}/services/data/v60.0/sobjects/{}", name)
    };
    raw
}

fn main() {
    println!("{}", build_url("Account", true));
}
