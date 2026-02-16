#![allow(missing_docs)]
use force::api::rest::search::SearchQueryBuilder;

#[test]
#[should_panic(expected = "sobject name contains invalid characters")]
fn test_sosl_injection_vulnerability_sobject() {
    let malicious_sobject = "Account; DROP TABLE";

    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning(malicious_sobject, &["Id"])
        .build();
}

#[test]
#[should_panic(expected = "field name contains invalid characters")]
fn test_sosl_injection_vulnerability_field() {
    let malicious_field = "Id) LIMIT 1000 --";

    let _ = SearchQueryBuilder::new()
        .find("test")
        .returning("Account", &[malicious_field])
        .build();
}
