use serde_json::Value;

pub enum BatchOp<'a> {
    Update(String, String, &'a Value),
    Delete(String, String),
    Create(String, &'a Value),
}
