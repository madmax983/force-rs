# Backtesting Engine

## The "So What?"
Traders need a reliable way to simulate and evaluate their trading strategies against historical market volatility before risking real capital.

## User Story
As a Trader, I want to backtest against volatile markets, so that I can validate my trading strategies and minimize financial risk.

## Metric Definition
Success = Backtest execution completes in under 1 minute for 10 years of historical data.

## Gap Analysis
Current tools lack seamless integration with our core data models and require exporting data to third-party platforms. A native backtesting engine would provide immediate, frictionless strategy validation.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
- Real-time execution (Phase 2).
