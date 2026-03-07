pub mod statements;

use std::fmt::{Display, Formatter};
use crate::statements::{BalanceSheetEntry, CashFlowEntry, IncomeStatementEntry};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Ticker {
    pub ticker: String,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingViewFinancials {
    pub ticker: String,
    pub currency: String,
    pub about: String,
    pub scraped_at: DateTime<Local>,

    pub quarterly_income: Vec<IncomeStatementEntry>,
    pub annual_income: Vec<IncomeStatementEntry>,

    pub quarterly_balance_sheet: Vec<BalanceSheetEntry>,
    pub annual_balance_sheet: Vec<BalanceSheetEntry>,

    pub quarterly_cash_flow: Vec<CashFlowEntry>,
    pub annual_cash_flow: Vec<CashFlowEntry>,

    pub ttm_income: Option<IncomeStatementEntry>,
    pub ttm_cash_flow: Option<CashFlowEntry>,
}

impl Ticker {
    pub fn new(exchange: impl Into<String>, ticker: impl Into<String>) -> Self {
        Self {
            exchange: exchange.into(),
            ticker: ticker.into(),
        }
    }
}

impl Display for Ticker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]", self.ticker, self.exchange)
    }
}
