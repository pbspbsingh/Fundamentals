mod client;
mod fetch;
mod parse;

pub use model::edgar::{Document, InsiderTransaction};

/// Fetch the 12 most recent 8-K filings for a ticker, with full text content.
pub async fn fetch_documents(ticker: &str) -> anyhow::Result<Vec<Document>> {
    let client = client::EdgarClient::new()?;
    fetch::fetch_documents(&client, ticker).await
}

/// Fetch insider (Form 4) transactions for the past 12 months.
pub async fn fetch_insider_transactions(ticker: &str) -> anyhow::Result<Vec<InsiderTransaction>> {
    let client = client::EdgarClient::new()?;
    fetch::fetch_insider_transactions(&client, ticker).await
}
