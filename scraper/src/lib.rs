mod financial_scraper;
mod screener_scraper;
mod sentiment_scraper;

use anyhow::Context;
use chrome_driver::{Browser, ChromeDriverConfig};
use model::ChromeConfig;

pub use financial_scraper::FinancialScraper;
pub use screener_scraper::ScreenerScraper;
pub use sentiment_scraper::SentimentScraper;

pub const TV_HOME: &str = "https://www.tradingview.com";

/// Launch browser using config from the `config` crate's OnceLock (standalone CLI use).
pub async fn launch_browser() -> anyhow::Result<Browser> {
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

/// Launch browser using a `ChromeConfig` supplied directly — no config crate OnceLock needed.
pub async fn launch_browser_with_config(chrome: &ChromeConfig) -> anyhow::Result<Browser> {
    let browser = ChromeDriverConfig::new(&chrome.chrome_path)
        .user_data_dir(&chrome.user_data_dir)
        .args(chrome.chrome_args.iter().map(|s| s.as_str()))
        .launch_if_needed(chrome.launch_if_needed)
        .connect()
        .await
        .with_context(|| {
            format!(
                "Failed to open browser with chrome_path='{}'",
                chrome.chrome_path.display()
            )
        })?;
    Ok(browser)
}
