use crate::TV_HOME;
use anyhow::Context;
use chrome_driver::{Browser, ChromeDriverConfig, Page, Sleepable};
use chrono::{Local, Utc};
use model::statements::IncomeStatementEntry;
use model::{Ticker, TradingViewFinancials};
use tracing::info;

pub struct FinancialScraper {
    browser: Option<Browser>,
    page: Option<Page>,
}

impl Drop for FinancialScraper {
    fn drop(&mut self) {
        if let Some(browser) = self.browser.take()
            && let Some(mut page) = self.page.take()
        {
            tokio::task::spawn(async move {
                page.close_me().await;
                drop(browser);
            });
        }
    }
}

impl FinancialScraper {
    pub async fn new() -> anyhow::Result<Self> {
        let browser = Self::launch_browser().await?;
        let page = browser.new_page(super::TV_HOME).await?;
        page.wait_for_navigation().await?.sleep().await;
        Ok(Self {
            browser: Some(browser),
            page: Some(page),
        })
    }

    pub async fn fetch_financials(&self, ticker: &Ticker) -> anyhow::Result<TradingViewFinancials> {
        info!("Navigating to financials page of {ticker}...");
        self.page()
            .goto(format!(
                "{TV_HOME}/symbols/{}-{}/financials-overview/",
                ticker.exchange, ticker.ticker
            ))
            .await?
            .wait_for_navigation()
            .await?
            .sleep()
            .await;

        let about = self.about().await?;

        self.page()
            .find_element("a#statements")
            .await?
            .click()
            .await?;
        self.page().sleep().await;

        let quarterly_income = self.parse_income_statement().await?;

        Ok(TradingViewFinancials {
            ticker: ticker.ticker.clone(),
            currency: "USD".into(),
            about,
            scraped_at: Local::now(),

            quarterly_income,
            annual_income: vec![],

            quarterly_balance_sheet: vec![],
            annual_balance_sheet: vec![],

            quarterly_cash_flow: vec![],
            annual_cash_flow: vec![],

            ttm_income: None,
            ttm_cash_flow: None,
        })
    }

    async fn about(&self) -> anyhow::Result<String> {
        Ok(self
            .page()
            .find_xpath("//p[starts-with(@class, 'description')]")
            .await?
            .inner_text()
            .await?
            .unwrap_or_default())
    }

    async fn parse_income_statement(&self) -> anyhow::Result<Vec<IncomeStatementEntry>> {
        anyhow::bail!("duh");
    }

    fn page(&self) -> &Page {
        self.page.as_ref().unwrap()
    }

    async fn launch_browser() -> anyhow::Result<Browser> {
        let cfg = config::config();
        let browser = ChromeDriverConfig::new(&cfg.chrome_path)
            .user_data_dir(&cfg.user_data_dir)
            .args(cfg.chrome_args.iter().map(|s| s.as_str()))
            .launch_if_needed(cfg.launch_if_needed)
            .connect()
            .await
            .with_context(|| format!("Failed to open browser with {cfg:?}"))?;
        Ok(browser)
    }
}
