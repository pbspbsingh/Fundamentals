mod financial_scraper;

use anyhow::Context;
use chrome_driver::{Browser, ChromeDriverConfig};
pub use financial_scraper::FinancialScraper;

pub const TV_HOME: &str = "https://www.tradingview.com";


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