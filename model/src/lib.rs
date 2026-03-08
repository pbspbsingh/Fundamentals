pub mod edgar;
pub mod financials;
pub mod sentiment;

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct Ticker {
    pub ticker: String,
    pub exchange: String,
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
