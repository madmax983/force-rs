# Volatile Market Backtesting

## Jobs to be Done
**User Story:** As a Trader, I want to backtest against volatile markets...

**The "So What?" ask:** What business problem does this solve? Traders need to validate strategies safely against unexpected market volatility.

**Metric Definition:** Success = Query latency < 10ms for 99% of requests.

**Gap Analysis:** Existing standard libraries panic on NaN data and don't provide easy CSV reporting for volatile slices.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
- Real-time execution (Phase 2).
