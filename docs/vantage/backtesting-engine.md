# Backtesting Engine

## Overview
A utility to evaluate trading strategies by simulating execution against historical market data.

## User Story
As a Trader, I want to backtest against volatile markets, so that I can validate my strategy's risk and profitability before trading with live capital.

## The "So What?"
**What business problem does this solve?**
Deploying unverified trading strategies into volatile markets carries immense financial risk. By simulating execution against historical data, we can empirically evaluate performance, catch edge cases, and tune parameters, thereby protecting capital and increasing expected returns.

## Metric Definition
- **Success =** Executes a full 10-year daily historical backtest in under 60 seconds with accurate simulation of slippage and commissions.

## Gap Analysis
Traders currently resort to exporting data to third-party tools or writing custom scripts for backtesting, which fragments the workflow and introduces errors. A native backtesting engine bridges this gap, allowing strategies to be seamlessly developed, tested, and eventually deployed in a unified ecosystem.

## Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## Out of Scope
- Real-time execution (Phase 2).
