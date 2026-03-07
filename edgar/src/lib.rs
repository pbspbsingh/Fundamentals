mod client;
mod compute;
mod fetch;
mod parse;
mod types;

pub use client::EdgarClient;
pub use types::*;

/// Fetch all EDGAR fundamental data for a ticker symbol.
pub async fn fetch_fundamentals(ticker: &str) -> anyhow::Result<EdgarFundamentals> {
    let client = EdgarClient::new()?;
    fetch::fetch_fundamentals(&client, ticker).await
}
