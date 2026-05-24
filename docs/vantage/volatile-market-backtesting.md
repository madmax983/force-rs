# Volatile Market Backtesting

## User Story
As a Trader, I want to backtest against volatile markets so that I can evaluate the robustness of my strategies during high turbulence.

## So What?
Our trading platform needs to prove that algorithmic strategies won't blow up during flash crashes or extreme volatility. This solves the business problem of strategy confidence and prevents massive capital loss during black swan events.

## Metric Definition
Success = Backtesting engine processes 10 years of tick data (including extreme volatility periods) in under 5 minutes, with zero panics on malformed data.

## Gap Analysis
Standard pandas/numpy backtesters often fail or silently propagate NaNs during extreme volatility, leading to false confidence. We need explicit, rust-backed NaN handling and error reporting that standard tools lack out-of-the-box.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
Real-time execution (Phase 2).
