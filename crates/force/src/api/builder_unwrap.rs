pub trait BuilderUnwrapExt<T> {
    fn unwrap_or_panic(self, context: &str) -> T;
}

impl<T> BuilderUnwrapExt<T> for Result<T, crate::error::ForceError> {
    fn unwrap_or_panic(self, context: &str) -> T {
        self.unwrap_or_else(|e| panic!("Invalid input in {}: {}", context, e))
    }
}
