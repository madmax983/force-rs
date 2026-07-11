pub trait BuilderUnwrapExt<T> {
    fn unwrap_or_panic(self, context: &str) -> T;
}

impl<T> BuilderUnwrapExt<T> for crate::error::Result<T> {
    fn unwrap_or_panic(self, context: &str) -> T {
        self.unwrap_or_else(|e| panic!("Invalid input in {}: {}", context, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ForceError;

    #[test]
    fn test_unwrap_or_panic_ok() {
        let result: crate::error::Result<i32> = Ok(42);
        assert_eq!(result.unwrap_or_panic("test_context"), 42);
    }

    #[test]
    #[should_panic(expected = "Invalid input in test_context: invalid input: test error")]
    fn test_unwrap_or_panic_err() {
        let result: crate::error::Result<i32> =
            Err(ForceError::InvalidInput("test error".to_string()));
        result.unwrap_or_panic("test_context");
    }
}
