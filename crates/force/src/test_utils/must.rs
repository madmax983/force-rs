use std::fmt::Debug;

pub trait Must<T> {
    fn must(self) -> T;
}

impl<T, E: Debug> Must<T> for std::result::Result<T, E> {
    fn must(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("unexpected Err: {error:?}"),
        }
    }
}

impl<T> Must<T> for Option<T> {
    fn must(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("unexpected None"),
        }
    }
}

pub trait MustMsg<T> {
    fn must_msg(self, message: &str) -> T;
}

impl<T, E: Debug> MustMsg<T> for std::result::Result<T, E> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error:?}"),
        }
    }
}

impl<T> MustMsg<T> for Option<T> {
    fn must_msg(self, message: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_must_result_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.must(), 42);
    }

    #[test]
    #[should_panic(expected = "unexpected Err: \"error message\"")]
    fn test_must_result_err() {
        let result: Result<i32, &str> = Err("error message");
        let _ = result.must();
    }

    #[test]
    fn test_must_option_some() {
        let option: Option<i32> = Some(42);
        assert_eq!(option.must(), 42);
    }

    #[test]
    #[should_panic(expected = "unexpected None")]
    fn test_must_option_none() {
        let option: Option<i32> = None;
        let _ = option.must();
    }

    #[test]
    fn test_must_msg_result_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.must_msg("Custom panic message"), 42);
    }

    #[test]
    #[should_panic(expected = "Custom panic message: \"error message\"")]
    fn test_must_msg_result_err() {
        let result: Result<i32, &str> = Err("error message");
        let _ = result.must_msg("Custom panic message");
    }

    #[test]
    fn test_must_msg_option_some() {
        let option: Option<i32> = Some(42);
        assert_eq!(option.must_msg("Custom panic message"), 42);
    }

    #[test]
    #[should_panic(expected = "Custom panic message")]
    fn test_must_msg_option_none() {
        let option: Option<i32> = None;
        let _ = option.must_msg("Custom panic message");
    }
}
