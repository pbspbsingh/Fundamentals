use std::sync::Arc;

use chrome_driver::{Browser, Page};
use model::{ChromeConfig, FetchConfig, StockFundamentals, Ticker};
use scraper::{FinancialScraper, ScreenerScraper, SentimentScraper, TV_HOME};
use tracing::info;

pub struct FundamentalsFetcher {
    _browser: Browser,
    page: Arc<Page>,
    sentiment_scraper: SentimentScraper,
    financial_scraper: FinancialScraper,
    screener_scraper: ScreenerScraper,
    edgar: edgar::EdgarClient,
}

impl FundamentalsFetcher {
    /// Create using the `config` crate's OnceLock (standalone CLI use).
    pub async fn new() -> anyhow::Result<Self> {
        let browser = scraper::launch_browser().await?;
        let page = Arc::new(browser.new_page(TV_HOME).await?);

        Ok(Self {
            financial_scraper: FinancialScraper::new(page.clone()).await,
            sentiment_scraper: SentimentScraper::new(page.clone()).await,
            screener_scraper: ScreenerScraper::new(page.clone()).await,
            edgar: edgar::EdgarClient::new()?,
            page,
            _browser: browser,
        })
    }

    /// Create using a `ChromeConfig` supplied directly — no config crate OnceLock needed.
    /// Used by WatchListManager which supplies its own config.
    pub async fn new_with_config(chrome: &ChromeConfig) -> anyhow::Result<Self> {
        let browser = scraper::launch_browser_with_config(chrome).await?;
        let page = Arc::new(browser.new_page(TV_HOME).await?);

        Ok(Self {
            financial_scraper: FinancialScraper::new(page.clone()).await,
            sentiment_scraper: SentimentScraper::new(page.clone()).await,
            screener_scraper: ScreenerScraper::new(page.clone()).await,
            edgar: edgar::EdgarClient::new()?,
            page,
            _browser: browser,
        })
    }

    /// Fetch all fundamentals sections (legacy API — used by standalone CLI).
    pub async fn fetch_fundamentals(&self, ticker: &Ticker) -> anyhow::Result<StockFundamentals> {
        self.fetch_fundamentals_with_config(ticker, &FetchConfig::default()).await
    }

    /// Fetch only the sections specified in `config`.
    pub async fn fetch_fundamentals_with_config(
        &self,
        ticker: &Ticker,
        config: &FetchConfig,
    ) -> anyhow::Result<StockFundamentals> {
        info!("Fetching fundamentals for {ticker} (config={config:?})");

        let sentiment = if config.sentiment {
            Some(self.sentiment_scraper.scrape(ticker).await?)
        } else {
            None
        };

        let financials = self
            .financial_scraper
            .fetch_financials_with_config(ticker, config)
            .await?;

        let documents = if config.sec_filings > 0 {
            let mut docs = self
                .edgar
                .fetch_documents(&ticker.ticker)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("EDGAR documents unavailable for {ticker}: {e:#}");
                    vec![]
                });
            docs.truncate(config.sec_filings);
            docs
        } else {
            vec![]
        };

        let insider_transaction = if config.insider_transactions {
            self.edgar
                .fetch_insider_transactions(&ticker.ticker)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("EDGAR insider transactions unavailable for {ticker}: {e:#}");
                    vec![]
                })
        } else {
            vec![]
        };

        Ok(StockFundamentals {
            ticker: ticker.clone(),
            sentiment,
            financials,
            documents,
            insider_transaction,
            last_updated: chrono::Utc::now(),
        })
    }

    /// Navigate to a TradingView screener and return the visible tickers.
    /// Only `exchange` and `ticker` fields are populated.
    pub async fn fetch_screener_tickers(
        &self,
        screener_url: &str,
    ) -> anyhow::Result<Vec<Ticker>> {
        self.screener_scraper.fetch_tickers(screener_url).await
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
