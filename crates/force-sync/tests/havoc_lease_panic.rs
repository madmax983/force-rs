//! 👺 Havoc: `PgStore` Lease Deadline Panic

#![allow(clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use deadpool_postgres::Config;
    use force_sync::PgStore;
    use std::time::Duration;

    #[tokio::test]
    async fn test_lease_duration_panic() {
        let mut config = Config::new();
        config.url = Some("postgresql://unused:unused@localhost:5432/unused".to_owned());
        let pool = config
            .create_pool(
                Some(deadpool_postgres::Runtime::Tokio1),
                tokio_postgres::NoTls,
            )
            .unwrap();
        let store = PgStore::new(pool);

        // This used to panic inside chrono::Duration::seconds.
        // The panic originates from chrono 0.4.44 `TimeDelta::seconds` when `secs` is very large but fits in i64.
        let result = store
            .lease_ready_tasks("worker", 10, Duration::from_secs(i64::MAX as u64))
            .await;

        // Now it should return a ForceSyncError::InvalidLeaseDuration
        assert!(matches!(
            result,
            Err(force_sync::ForceSyncError::InvalidLeaseDuration)
        ));
    }
}
