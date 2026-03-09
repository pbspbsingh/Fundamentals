use std::sync::Arc;

use chrome_driver::{Browser, Page, Sleepable};
use model::{StockFundamentals, Ticker};
use scraper::{FinancialScraper, SentimentScraper, TV_HOME};
use tracing::info;

pub struct FundamentalsFetcher {
    _browser: Browser,
    page: Arc<Page>,
    sentiment_scraper: SentimentScraper,
    financial_scraper: FinancialScraper,
    edgar: edgar::EdgarClient,
}

impl FundamentalsFetcher {
    pub async fn new() -> anyhow::Result<Self> {
        let browser = scraper::launch_browser().await?;
        let page = Arc::new(browser.new_page(TV_HOME).await?);

        Ok(Self {
            financial_scraper: FinancialScraper::new(page.clone()).await,
            sentiment_scraper: SentimentScraper::new(page.clone()).await,
            edgar: edgar::EdgarClient::new()?,
            page,
            _browser: browser,
        })
    }

    pub async fn fetch_fundamentals(&self, ticker: &Ticker) -> anyhow::Result<StockFundamentals> {
        info!("Fetching fundamentals for {ticker}");

        // Browser scrapers share one page — run sequentially to avoid navigation conflicts.
        let sentiment = self.sentiment_scraper.scrape(ticker).await?;
        let financials = self.financial_scraper.fetch_financials(ticker).await?;

        let documents = edgar::fetch_documents(&ticker.ticker).await.unwrap_or_else(|e| {
            tracing::warn!("EDGAR documents unavailable for {ticker}: {e:#}");
            vec![]
        });
        let insider_transaction =
            edgar::fetch_insider_transactions(&ticker.ticker).await.unwrap_or_else(|e| {
                tracing::warn!("EDGAR insider transactions unavailable for {ticker}: {e:#}");
                vec![]
            });

        Ok(StockFundamentals {
            ticker: ticker.clone(),
            sentiment,
            financials,
            documents,
            insider_transaction,
            last_updated: chrono::Utc::now(),
        })
    }
}

impl Drop for FundamentalsFetcher {
    fn drop(&mut self) {
        use chrome_driver::chromiumoxide::cdp::browser_protocol::target::CloseTargetParams;

        let page = self.page.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // block_in_place suspends the current async task so block_on can run.
            tokio::task::block_in_place(|| {
                handle.block_on(async move {
                    let target_id = page.target_id().clone();
                    let _ = page.execute(CloseTargetParams::new(target_id)).await;
                });
            });
        }
    }
}
