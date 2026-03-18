import json

data = {
    "module": "crates/force/src/client/mod.rs",
    "tests": [
        {
            "name": "test_builder_creates_noauth_state",
            "reason": "It does not actually check anything. The body just has a comment."
        },
        {
            "name": "test_force_client_clone",
            "reason": "It tests cloning but doesn't test the other methods on ForceClient, leaving them exposed to mutations."
        }
    ]
}
