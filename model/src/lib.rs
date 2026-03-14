pub mod edgar;
pub mod financials;
pub mod sentiment;

use crate::edgar::{Document, InsiderTransaction};
use crate::financials::TradingViewFinancials;
use crate::sentiment::StockSentiment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

/// Chrome browser configuration — passed directly to `FundamentalsFetcher::new()`.
/// WatchListManager uses this instead of the `config` crate's OnceLock.
#[derive(Debug, Clone)]
pub struct ChromeConfig {
    pub chrome_path:      PathBuf,
    pub user_data_dir:    PathBuf,
    pub chrome_args:      Vec<String>,
    pub launch_if_needed: bool,
}

/// Controls which sections `fetch_fundamentals` actually scrapes.
/// Only sections with a `true` flag (or non-zero `sec_filings`) are fetched.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub sentiment:             bool,
    pub income_statement:      bool,
    pub balance_sheet:         bool,
    pub cash_flow:             bool,
    pub statistics:            bool,
    pub earnings:              bool,
    /// How many 8-K documents to fetch (0 = skip entirely).
    pub sec_filings:           usize,
    pub insider_transactions:  bool,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            sentiment:            true,
            income_statement:     false,
            balance_sheet:        false,
            cash_flow:            false,
            statistics:           false,
            earnings:             true,
            sec_filings:          1,
            insider_transactions: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub ticker: String,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockFundamentals {
    pub ticker: Ticker,
    /// `None` when `FetchConfig::sentiment = false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentiment: Option<StockSentiment>,
    pub financials: TradingViewFinancials,
    /// Empty when `FetchConfig::sec_filings = 0`.
    pub documents: Vec<Document>,
    /// Empty when `FetchConfig::insider_transactions = false`.
    pub insider_transaction: Vec<InsiderTransaction>,
    pub last_updated: DateTime<Utc>,
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
