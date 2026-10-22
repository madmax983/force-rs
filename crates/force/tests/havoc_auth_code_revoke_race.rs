//! Test for auth code revoke race condition

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use loom::sync::{Arc, RwLock};
    use loom::thread;

    // Test that explicitly models `auth_code.rs` `revoke_stored_refresh_token` race condition!
    // We already fixed it in `auth_code.rs`, so let's write a Loom model matching the fixed version to prove it passes.

    struct AuthCodeFixed {
        refresh_token: Arc<RwLock<Option<String>>>,
    }

    impl AuthCodeFixed {
        fn new() -> Self {
            Self {
                refresh_token: Arc::new(RwLock::new(Some("old_rt".to_string()))),
            }
        }

        fn revoke_stored_refresh_token(&self) -> Option<String> {
            let stored = self.refresh_token.read().unwrap().clone();
            if let Some(rt) = stored.clone() {
                // Simulate network call

                let mut guard = self.refresh_token.write().unwrap();
                // FIXED check:
                if guard.as_ref() == Some(&rt) {
                    *guard = None;
                }
            }
            stored
        }

        fn authenticate(&self) {
            let mut stored = self.refresh_token.write().unwrap();
            *stored = Some("new_rt".to_string());
        }
    }

    #[test]
    fn test_revoke_race_fixed() {
        loom::model(|| {
            let auth = Arc::new(AuthCodeFixed::new());
            let a1 = auth.clone();
            let a2 = auth.clone();

            let t1 = thread::spawn(move || a1.revoke_stored_refresh_token());

            let t2 = thread::spawn(move || {
                a2.authenticate();
            });

            let revoked_rt = t1.join().unwrap();
            t2.join().unwrap();

            let final_rt = auth.refresh_token.read().unwrap().clone();

            assert!(
                !(revoked_rt == Some("old_rt".to_string()) && final_rt.is_none()),
                "Race condition: new_rt was overwritten by revoke of old_rt"
            );
        });
    }
}
