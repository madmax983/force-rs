pub trait BuilderUnwrapExt<T> {
    fn unwrap_or_panic(self, context: &str) -> T;
}

impl<T> BuilderUnwrapExt<T> for Result<T, crate::error::ForceError> {
    fn unwrap_or_panic(self, context: &str) -> T {
        self.unwrap_or_else(|e| panic!("Invalid input in {}: {}", context, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ForceError;

    #[test]
    #[should_panic(expected = "Invalid input in my_context: invalid input: test error")]
    fn test_unwrap_or_panic_formats_correctly() {
        let result: Result<(), ForceError> =
            Err(ForceError::InvalidInput("test error".to_string()));
        result.unwrap_or_panic("my_context");
    }

    #[test]
    fn test_unwrap_or_panic_returns_ok_value() {
        let result: Result<i32, ForceError> = Ok(42);
        assert_eq!(result.unwrap_or_panic("my_context"), 42);
    }
}
