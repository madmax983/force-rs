# Backtesting Spec

## The 'So What?'
We need to validate trading strategies against historical data to estimate potential returns and drawdowns before deploying capital into live markets. Without a reliable backtester, traders are essentially guessing and risking severe losses.

## User Story
As a Trader, I want to backtest against volatile markets, so that I can evaluate the robustness of my algorithms.

## Metric Definition
- Success = Process 10 years of tick data in under 5 minutes.
- Accurate reproduction of trades matching standard references (zero unexpected deviations).

## Gap Analysis
Existing solutions either require moving data out of the ecosystem (costly & slow) or are too tightly coupled to specific execution venues. We need a fast, local engine that integrates with our data pipeline.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report detailing trades, drawdowns, and Sharpe ratio.

## Out of Scope
Real-time execution (Phase 2).
