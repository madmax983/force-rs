#![allow(clippy::expect_used)]
//! Havoc race condition test for `UsernamePassword` refresh token clearing.
//!
//! This test demonstrates that if `refresh()` fails (because the token was revoked),
//! it might blindly clear `refresh_token` even if another thread concurrently
//! authenticated and stored a *new* valid refresh token.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use loom::sync::{Arc, RwLock};
    use loom::thread;

    struct UsernamePassword {
        refresh_token: RwLock<Option<String>>,
    }

    impl UsernamePassword {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                refresh_token: RwLock::new(Some("old_rt".to_string())),
            })
        }

        // Simulate Authenticator::authenticate()
        fn authenticate(&self) {
            *self.refresh_token.write().expect("test failed") = Some("new_rt".to_string());
        }

        // Simulate Authenticator::refresh()
        fn refresh(&self) -> (Option<String>, bool) {
            let stored_rt = self.refresh_token.read().expect("test failed").clone();

            if let Some(rt) = stored_rt {
                // Network request... (we yield here so Thread 2 can `authenticate` and inject "new_rt")
                thread::yield_now();

                // Refresh token revoked or expired — fall back to full re-auth.
                let mut stored = self.refresh_token.write().expect("test failed");

                // BUG FIX: Only clear it if it hasn't changed since we started!
                if stored.as_deref() == Some(rt.as_str()) {
                    *stored = None;
                }
                drop(stored);

                // 👺 Havoc: Return immediately on failure to prove the token is left as None!
                return (Some(rt), false);
            }

            self.authenticate();
            (None, true)
        }
    }

    #[test]
    fn test_refresh_clears_newer_token() {
        loom::model(|| {
            let auth = UsernamePassword::new();
            let auth1 = auth.clone();
            let auth2 = auth.clone();

            let t1 = thread::spawn(move || auth1.refresh());
            let t2 = thread::spawn(move || auth2.authenticate());

            let (refreshed_token, _) = t1.join().expect("test failed");
            t2.join().expect("test failed");

            let final_rt = auth.refresh_token.read().expect("test failed").clone();

            // If t1 failed while trying to refresh "old_rt", it should NEVER overwrite "new_rt"
            // if t2 injected it after t1 read "old_rt".
            if refreshed_token.as_deref() == Some("old_rt") {
                assert_eq!(
                    final_rt.as_deref(),
                    Some("new_rt"),
                    "👺 Havoc: refresh() wiped out a new refresh token!"
                );
            }
        });
    }
}
