#![allow(missing_docs)]
#![cfg(feature = "rest")]

use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "Invalid sobject name")]
fn test_sosl_injection_in_sobject() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account) OR (Contact", &[] as &[&str])
        .build();
}

#[test]
#[should_panic(expected = "Invalid field expression")]
fn test_sosl_injection_in_fields() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id) LIMIT 1 --"])
        .build();
}

#[test]
#[should_panic(expected = "Invalid field expression")]
fn test_sosl_injection_unbalanced_paren() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id)"])
        .build();
}

#[test]
#[should_panic(expected = "Invalid field expression")]
fn test_sosl_injection_break_out_with_comma() {
    // This attempts to close the object and start a new one
    let _ = SearchQueryBuilder::new()
        .find("test")
        // "Id), Contact(Id" would result in "Account(Id), Contact(Id))" if injected
        .returning("Account", &["Id), Contact(Id"])
        .build();
}

#[test]
fn test_valid_complex_fields() {
    let query = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["toLabel(Industry)", "convertCurrency(Amount)"])
        .build();

    assert_eq!(
        query,
        "FIND {test} RETURNING Account(toLabel(Industry), convertCurrency(Amount))"
    );
}

#[test]
fn test_valid_quoted_where_clause() {
    // SOSL allows WHERE clauses inside the field spec
    let query = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id WHERE Name = 'Acme)'"])
        .build();

    // The validator should allow ')' inside quotes
    assert_eq!(
        query,
        "FIND {test} RETURNING Account(Id WHERE Name = 'Acme)')"
    );
}

#[test]
#[should_panic(expected = "Invalid field expression")]
fn test_invalid_unclosed_quote() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id WHERE Name = 'Acme"])
        .build();
}

#[test]
#[should_panic(expected = "Invalid field expression")]
fn test_invalid_trailing_backslash() {
    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &["Id\\"])
        .build();
}
