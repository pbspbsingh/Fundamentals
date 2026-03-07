use chrome_driver::{Browser, ChromeDriverConfig, Sleepable};
use model::Ticker;



pub async fn start_fetching(ticker: &Ticker) -> anyhow::Result<()> {
    let browser = launch_browser().await?;
    let page = browser.new_page(format!("https://www.tradingview.com/symbols/{}-{}/", ticker.exchange, ticker.ticker)).await?;
    Ok(())
}

async fn launch_browser() -> anyhow::Result<Browser> {
    let cfg = config::config();
    let browser = ChromeDriverConfig::new(&cfg.chrome_path)
        .user_data_dir(&cfg.user_data_dir)
        .args(cfg.chrome_args.iter().map(|s| s.as_str()))
        .launch_if_needed(cfg.launch_if_needed)
        .connect()
        .await?;

    Ok(browser)
}