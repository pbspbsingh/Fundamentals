mod client;
mod fetch;
mod parse;

pub use client::EdgarClient;

pub use model::edgar::{Document, InsiderTransaction, InstitutionalHolder};

impl EdgarClient {
    /// Fetch the 12 most recent 8-K filings for a ticker, with full text content.
    pub async fn fetch_documents(&self, ticker: &str) -> anyhow::Result<Vec<Document>> {
        fetch::fetch_documents(self, ticker).await
    }

    /// Fetch insider (Form 4) transactions for the past 12 months.
    pub async fn fetch_insider_transactions(
        &self,
        ticker: &str,
    ) -> anyhow::Result<Vec<InsiderTransaction>> {
        fetch::fetch_insider_transactions(self, ticker).await
    }

    /// Fetch institutional holders (13F-HR) for the most recent quarter.
    pub async fn fetch_institutional_holders(
        &self,
        ticker: &str,
    ) -> anyhow::Result<Vec<InstitutionalHolder>> {
        fetch::fetch_institutional_holders(self, ticker).await
    }
}
