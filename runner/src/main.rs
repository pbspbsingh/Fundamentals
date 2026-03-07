use std::time::Duration;
use time::macros::format_description;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::OffsetTime;
use model::Ticker;
use scraper::FinancialScraper;

fn main() -> anyhow::Result<()> {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let timer = OffsetTime::new(offset, format_description!("[hour]:[minute]:[second]"));
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config::config().rust_log));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_timer(timer)
        .with_writer(std::io::stderr)
        .init();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    info!("Starting the app with config:\n{:#?}", config::config());
    let scraper = FinancialScraper::new().await?;
    if let Err(e) = scraper.fetch_financials(&Ticker::new("NASDAQ", "PLTR")).await {
        error!("Error fetching financials: {}", e);
    }
    drop(scraper);
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}
