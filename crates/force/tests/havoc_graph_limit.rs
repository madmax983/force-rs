//! Havoc test for Composite Graph node limits.

#![cfg(feature = "composite")]
#![allow(clippy::unwrap_used)]
#![cfg(feature = "composite_graph")]

use force::api::composite::{Graph, GraphRequest};

#[test]
fn test_havoc_graph_limit_enforced() {
    let mut graph = Graph::new("graph1");

    // Add 500 requests, which is the maximum limit.
    for i in 0..500 {
        let req = GraphRequest::new("GET", "/sobjects/Account", format!("ref_{i}"));
        graph = graph.add_request(req).unwrap();
    }

    assert_eq!(graph.composite_request.len(), 500);

    // The 501st request MUST fail.
    let req = GraphRequest::new("GET", "/sobjects/Account", "ref_501");
    let result = graph.add_request(req);

    assert!(
        result.is_err(),
        "👺 Havoc: Graph allowed more than 500 requests, violating Salesforce limits!"
    );
}
