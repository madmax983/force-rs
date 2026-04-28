# Backtesting Engine

## 👤 User Story
As a Trader, I want to backtest against volatile markets, so that I can validate my trading strategies under different market conditions.

## ❓ So What?
What business problem does this solve? It allows traders to predict how their strategies will perform and optimize them to reduce losses and maximize profits before executing them with real money. This increases confidence and adoption of the trading platform.

## 🎯 Success Metrics
- Query latency < 10ms for 99% of requests.
- Can process 1 year of historical tick data in under 5 minutes.

## 🔍 Gap Analysis
Currently, traders have to manually export data to CSV and write custom scripts in Python or Excel to backtest. There is no integrated engine in our platform to run simulations natively against our historical database.

## ✅ Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.
- Must support loading historical data sets by date range.

## 🚫 Out of Scope
- Real-time execution (Phase 2).
- Machine learning model generation.
- Connecting to external live brokers for trading.
