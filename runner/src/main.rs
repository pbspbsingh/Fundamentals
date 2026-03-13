use model::Ticker;
use runner::fundamentals_fetcher::FundamentalsFetcher;
use time::macros::format_description;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::OffsetTime;

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
    let mut args = std::env::args();
    let (Some(exchange), Some(stock)) = (args.nth(1), args.next()) else {
        anyhow::bail!("Requires {{exchange}} {{stock}}");
    };
    info!("Scraping fundamentals of {exchange}:{stock}");
    let fetcher = FundamentalsFetcher::new().await?;
    let fundamentals = fetcher
        .fetch_fundamentals(&Ticker::new(exchange, stock))
        .await?;
    tokio::fs::write(
        "fundamentals.json",
        serde_json::to_string_pretty(&fundamentals)?,
    )
    .await?;
    Ok(())
}
