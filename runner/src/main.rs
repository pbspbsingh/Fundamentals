

use time::macros::format_description;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::OffsetTime;
use model::Ticker;
use runner::fundamentals_fetcher::FundamentalsFetcher;

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
    let fetcher = FundamentalsFetcher::new().await?;
    let fundamentals = fetcher.fetch_fundamentals(&Ticker::new("NASDAQ", "TSLA")).await?;
    tokio::fs::write("fundamentals.json", serde_json::to_string_pretty(&fundamentals)?).await?;
    Ok(())
}
