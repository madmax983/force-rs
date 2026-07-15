//! Configuration types for the Salesforce Pub/Sub API client.

use std::time::Duration;

/// Configuration for the Pub/Sub client.
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    /// gRPC endpoint for the Pub/Sub API.
    pub endpoint: String,
    /// Number of events to request per FetchRequest batch.
    ///
    /// Must be between 1 and 100 (inclusive). Values outside this range will
    /// cause `PubSubHandler::connect` to return a `PubSubError::Config` error.
    /// The Salesforce Pub/Sub API rejects requests with 0 or negative values.
    pub batch_size: i32,
    /// Reconnection policy for the subscribe stream.
    pub reconnect_policy: ReconnectPolicy,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.pubsub.salesforce.com:7443".to_string(),
            batch_size: 100,
            reconnect_policy: ReconnectPolicy::default(),
        }
    }
}

/// Controls reconnection behaviour when the subscribe stream drops.
#[derive(Debug, Clone)]
pub enum ReconnectPolicy {
    /// No automatic reconnection — errors surface to the stream consumer.
    None,
    /// Reconnect automatically, resuming from the last seen replay ID.
    Auto {
        /// Maximum number of reconnection attempts before giving up.
        max_retries: u32,
        /// Backoff configuration between attempts.
        backoff: BackoffConfig,
    },
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::Auto {
            max_retries: 5,
            backoff: BackoffConfig::default(),
        }
    }
}

/// Exponential backoff with configurable parameters.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial delay before the first retry.
    pub initial_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Multiplier applied after each attempt.
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl BackoffConfig {
    /// Compute the delay for a given attempt number (0-indexed).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let exp = attempt.min(63) as i32;
        let multiplied = self.initial_delay.as_secs_f64() * self.multiplier.powi(exp);
        let capped = multiplied.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    }
}

/// Where to start replaying events when subscribing.
#[derive(Debug, Clone)]
pub enum ReplayPreset {
    /// Only events published after subscribing.
    Latest,
    /// All events within the 72-hour retention window.
    Earliest,
    /// Resume from a specific replay ID.
    Custom(crate::types::ReplayId),
}

#[cfg(test)]
#[allow(clippy::no_effect_underscore_binding)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PubSubConfig::default();
        assert_eq!(config.endpoint, "https://api.pubsub.salesforce.com:7443");
        assert_eq!(config.batch_size, 100);
        assert!(matches!(
            config.reconnect_policy,
            ReconnectPolicy::Auto { .. }
        ));
    }

    #[test]
    fn test_default_reconnect_policy() {
        let policy = ReconnectPolicy::default();
        match policy {
            ReconnectPolicy::Auto { max_retries, .. } => assert_eq!(max_retries, 5),
            ReconnectPolicy::None => panic!("expected Auto"),
        }
    }

    #[test]
    fn test_backoff_delay_for_attempt_0() {
        let backoff = BackoffConfig::default();
        assert_eq!(backoff.delay_for(0), Duration::from_millis(500));
    }

    #[test]
    fn test_backoff_delay_for_attempt_1() {
        let backoff = BackoffConfig::default();
        assert_eq!(backoff.delay_for(1), Duration::from_secs(1));
    }

    #[test]
    fn test_backoff_delay_capped_at_max() {
        let backoff = BackoffConfig::default();
        let delay = backoff.delay_for(20);
        assert_eq!(delay, Duration::from_secs(30));
    }

    #[test]
    fn test_replay_preset_variants_exist() {
        let _latest = ReplayPreset::Latest;
        let _earliest = ReplayPreset::Earliest;
    }
}
