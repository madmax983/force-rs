# 🔭 Vantage: Spec for Backtesting

**Business problem:**
Quantitative traders and analysts need to validate algorithms against historical market volatility before deploying them with real capital.

**Gap Analysis:**
Currently, our platform lacks a native backtesting engine. Traders are forced to export data and use third-party tools, which breaks the workflow and introduces latency in the research phase.

**Metric Definition:**
Success = Query latency < 10ms for 99% of requests during backtest simulations.

👤 **User Story:**
As a Trader, I want to backtest against volatile markets, so that I can validate my algorithm's risk management without losing real capital.

✅ **Acceptance Criteria:**
- Must handle NaN data without panicking.
- Must output a CSV report.

🚫 **Out of Scope:**
- Real-time execution (Phase 2).
