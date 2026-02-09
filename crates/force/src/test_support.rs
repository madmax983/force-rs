//! Test-only helper utilities for ergonomic assertions without `unwrap`/`expect`.

#[cfg(test)]
use core::fmt::Debug;

/// Extension trait for unwrapping `Result`/`Option` in tests without `unwrap()`.
#[cfg(test)]
pub trait Must<T> {
    /// Extracts the inner value or panics with a default diagnostic message.
    fn must(self) -> T;
}

#[cfg(test)]
impl<T, E: Debug> Must<T> for Result<T, E> {
    fn must(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("unexpected Err: {error:?}"),
        }
    }
}

#[cfg(test)]
impl<T> Must<T> for Option<T> {
    fn must(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("unexpected None"),
        }
    }
}

/// Extension trait for unwrapping with custom panic messages.
#[cfg(test)]
pub trait MustMsg<T> {
    /// Extracts the inner value or panics with `message`.
    fn must_msg(self, message: &str) -> T;
}

#[cfg(test)]
impl<T, E: Debug> MustMsg<T> for Result<T, E> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error:?}"),
        }
    }
}

#[cfg(test)]
impl<T> MustMsg<T> for Option<T> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{message}"),
        }
    }
}

