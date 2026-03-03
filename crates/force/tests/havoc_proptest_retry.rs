#[cfg(test)]
mod proptests {
    use proptest::prelude::*;
    use std::time::Duration;
    // include the module directly or call it if it's public?
    // Wait, exponential_backoff is pub(crate). So we can't test it from tests/ directory directly unless we use pub(crate).
}
