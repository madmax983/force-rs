//! Tests for panics in `SoqlQueryBuilder` due to unhandled invalid input.

use force::api::SoqlQueryBuilder;

#[test]
#[should_panic(expected = "Invalid input in from: invalid input: SObject name cannot be empty")]
fn havoc_builder_empty_from() {
    let _ = SoqlQueryBuilder::new().from("");
}
