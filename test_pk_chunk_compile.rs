use force::api::bulk::query::BulkQueryRequest;

fn main() {
    let req = BulkQueryRequest::new("SELECT Id FROM Account");
}
