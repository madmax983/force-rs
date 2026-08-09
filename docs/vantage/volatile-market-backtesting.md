# Volatile Market Backtesting

## Overview
A spec for backtesting against volatile markets.

## User Story
As a Trader, I want to backtest against volatile markets, so that I can understand risk and performance under stress.

## The "So What?"
**What business problem does this solve?**
Traders need to simulate strategies under extreme market conditions to ensure their algorithms survive high volatility events.

## Metric Definition
- **Success =** Strategy executes within backtest environment correctly without panicking on dirty data.

## Gap Analysis
Current backtesting doesn't effectively simulate NaN and volatile conditions.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
- Real-time execution (Phase 2).
