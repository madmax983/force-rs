use crate::api::rest::describe::FieldDescribe;

/// Collects and sorts fields alphabetically, ensuring the 'Id' field is always first if it exists.
pub fn sort_fields_id_first<'a>(
    fields: impl Iterator<Item = &'a FieldDescribe>,
) -> Vec<&'a FieldDescribe> {
    let mut sorted_fields: Vec<&_> = fields.collect();
    sorted_fields.sort_by(|a, b| {
        if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });
    sorted_fields
}
