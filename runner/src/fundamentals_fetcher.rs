use std::sync::Arc;
use chrome_driver::{Browser, Page};
use model::{StockFundamentals, Ticker};
use scraper::{FinancialScraper, SentimentScraper};

pub struct FundamentalsFetcher {
    browser: Browser,
    sentiment_scraper: SentimentScraper,
    financial_scraper: FinancialScraper,
    edgar: edgar::EdgarClient,
}

impl FundamentalsFetcher {
    pub fn new() -> anyhow::Result<Self> {
        todo!()
    }

    pub fn fetch_fundamentals(&self, ticker: &Ticker) -> anyhow::Result<StockFundamentals> {
        todo!()
    }
}