pub mod edgar;
pub mod financials;
pub mod sentiment;

use std::fmt::{Display, Formatter};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::edgar::{Document, InsiderTransaction, InstitutionalHolder};
use crate::financials::TradingViewFinancials;
use crate::sentiment::StockSentiment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub ticker: String,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockFundamentals {
    pub ticker: Ticker,
    pub sentiment: StockSentiment,
    pub financials: TradingViewFinancials,
    pub documents: Vec<Document>,
    pub insider_transaction: Vec<InsiderTransaction>,
    pub institutional_holders: Vec<InstitutionalHolder>,
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
