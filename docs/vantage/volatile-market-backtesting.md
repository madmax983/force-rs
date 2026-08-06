# 🔭 Vantage: Spec for Volatile Market Backtesting

**What business problem does this solve?**
Traders need to accurately simulate the behavior of their algorithmic strategies during extreme market volatility.

👤 **User Story:**
As a Trader, I want to backtest against volatile markets, so that I can understand my strategy's drawdown risks under stress.

✅ **Acceptance Criteria:**
- Must handle NaN data without panicking.
- Must output a CSV report.

🚫 **Out of Scope:**
- Real-time execution (Phase 2).
