## [Reduction]
**Bloat:** RetryPolicy, RequestRetryClass, TelemetryHooks (over-engineered retry logic and unused telemetry)
**Cut:** Simplified HttpExecutor with simple boolean retry flag
**Saved:** ~200 lines of code / reduced cognitive load
