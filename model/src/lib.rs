#[derive(Debug, Clone)]
pub struct Ticker {
    pub ticker: String,
    pub exchange: String,
}

impl Ticker {
    pub fn new(exchange: &str, ticker: &str) -> Self {
        Self {
            exchange: exchange.to_string(),
            ticker: ticker.to_string(),
        }
    }
}
