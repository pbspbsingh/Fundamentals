use time::macros::format_description;
use tracing::{error, info};
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
    let ticker = "NVDA";

    info!("Fetching 8-K documents for {ticker}...");
    match edgar::fetch_documents(ticker).await {
        Err(e) => error!("fetch_documents failed: {e:#}"),
        Ok(docs) => {
            info!("Got {} documents", docs.len());
            tokio::fs::write("documents.json", serde_json::to_string_pretty(&docs)?).await?;
        }
    }

    info!("Fetching insider transactions for {ticker}...");
    match edgar::fetch_insider_transactions(ticker).await {
        Err(e) => error!("fetch_insider_transactions failed: {e:#}"),
        Ok(txs) => {
            info!("Got {} insider transactions", txs.len());
            tokio::fs::write("insider.json", serde_json::to_string_pretty(&txs)?).await?;
        }
    }

    Ok(())
}
