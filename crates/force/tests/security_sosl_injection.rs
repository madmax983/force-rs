#![allow(missing_docs)]
#![cfg(feature = "rest")]

use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "Invalid object name")]
fn test_sosl_injection_object_name() {
    // This input attempts to close the object clause and start a new one
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account), Contact(Id", &["Name"])
        .build();
}

#[test]
#[should_panic(expected = "Invalid field syntax")]
fn test_sosl_injection_field_unbalanced() {
    // This input attempts to close the object clause via a field
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id), Contact(Name"])
        .build();
}

#[test]
#[should_panic(expected = "Invalid object name")]
fn test_sosl_injection_object_weird_chars() {
    // Hyphens are not allowed in object names
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account-Bad", &["Name"])
        .build();
}

#[test]
fn test_sosl_valid_tolabel() {
    // Valid usage of function in field list
    let query = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["toLabel(Industry)"])
        .build();
    assert!(query.contains("Account(toLabel(Industry))"));
}

#[test]
fn test_sosl_valid_relationship() {
    // Valid relationship field
    let query = SearchQueryBuilder::new()
        .find("test")
        .returning("Contact", &["Account.Name"])
        .build();
    assert!(query.contains("Contact(Account.Name)"));
}

#[test]
fn test_sosl_valid_complex_where() {
    // Valid usage of WHERE and ORDER BY in field list
    // RETURNING Account(Name WHERE CreatedDate > TODAY ORDER BY Name DESC)
    let query = SearchQueryBuilder::new()
        .find("test")
        .returning(
            "Account",
            &["Name WHERE CreatedDate > TODAY ORDER BY Name DESC"],
        )
        .build();
    assert!(query.contains("Account(Name WHERE CreatedDate > TODAY ORDER BY Name DESC)"));
}
