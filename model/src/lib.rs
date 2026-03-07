use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Fundamentals {
    pub description: String,
    pub last_earnings_date: NaiveDate,
    pub ipo_date: NaiveDate,
}

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
