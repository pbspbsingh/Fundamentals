use std::{path::PathBuf, time::Duration};

use anyhow::{Context, bail};
use serde::de::DeserializeOwned;
use tokio::time::sleep;

const BASE_URL: &str = "https://data.sec.gov";
const TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";
const TICKERS_CACHE_PATH: &str = "company_tickers_cache.json";
const CACHE_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60; // 7 days
const RATE_LIMIT_DELAY: Duration = Duration::from_millis(110);

pub struct EdgarClient {
    pub(crate) http: reqwest::Client,
}

impl EdgarClient {
    pub fn new() -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("Fundamentals pbspbsingh@gmail.com")
            .build()?;
        Ok(Self { http })
    }

    /// Resolve a ticker symbol to a zero-padded 10-digit CIK string and company name.
    pub async fn resolve_cik(&self, ticker: &str) -> anyhow::Result<(String, String)> {
        let ticker_upper = ticker.to_uppercase();

        // The tickers JSON is a map of index -> {cik_str, ticker, title}
        let raw: serde_json::Value = self.load_tickers_json().await?;

        let map = raw.as_object().context("expected JSON object")?;
        for entry in map.values() {
            let t = entry["ticker"].as_str().unwrap_or("");
            if t.eq_ignore_ascii_case(&ticker_upper) {
                let cik_num = entry["cik_str"]
                    .as_u64()
                    .or_else(|| {
                        entry["cik_str"]
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok())
                    })
                    .context("missing cik_str")?;
                let company_name = entry["title"].as_str().unwrap_or("").to_string();
                let cik = format!("CIK{cik_num:010}");
                return Ok((cik, company_name));
            }
        }

        bail!("ticker '{ticker}' not found in SEC company_tickers.json")
    }

    /// Fetch `{BASE_URL}/submissions/{cik}.json`
    pub async fn fetch_submissions(&self, cik: &str) -> anyhow::Result<serde_json::Value> {
        let url = format!("{BASE_URL}/submissions/{cik}.json");
        self.get_json(&url)
            .await
            .with_context(|| format!("fetching submissions for {cik}"))
    }

    /// Fetch any EDGAR filing document by raw (numeric) CIK, accession number, and filename.
    /// URL: https://www.sec.gov/Archives/edgar/data/{raw_cik}/{accn_no_dashes}/{filename}
    pub async fn fetch_filing_text(
        &self,
        raw_cik: &str,
        accession: &str,
        filename: &str,
    ) -> anyhow::Result<String> {
        let accession_no_dashes = accession.replace('-', "");
        // Some primaryDocument paths include a stylesheet prefix (e.g. "xslF345X05/foo.xml").
        // The actual document always lives at the root of the accession folder.
        let doc_name = filename.rsplit('/').next().unwrap_or(filename);
        let url = format!(
            "https://www.sec.gov/Archives/edgar/data/{raw_cik}/{accession_no_dashes}/{doc_name}"
        );

        sleep(RATE_LIMIT_DELAY).await;
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("fetching filing at {url}"))?;

        if !resp.status().is_success() {
            bail!("Filing fetch failed: HTTP {} for {url}", resp.status());
        }
        Ok(resp.text().await?)
    }

    /// Load company_tickers.json from local cache if fresh, otherwise fetch and cache it.
    async fn load_tickers_json(&self) -> anyhow::Result<serde_json::Value> {
        let cache_path = PathBuf::from(TICKERS_CACHE_PATH);

        if let Ok(meta) = std::fs::metadata(&cache_path) {
            let age = meta
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs())
                .unwrap_or(u64::MAX);

            if age < CACHE_MAX_AGE_SECS {
                let bytes = std::fs::read(&cache_path).context("reading company_tickers cache")?;
                return serde_json::from_slice(&bytes)
                    .context("parsing cached company_tickers.json");
            }
        }

        let raw: serde_json::Value = self
            .get_json(TICKERS_URL)
            .await
            .context("fetching company_tickers.json")?;

        // Write cache (best-effort — ignore errors so a read-only CWD doesn't break things)
        if let Ok(bytes) = serde_json::to_vec(&raw) {
            let _ = std::fs::write(&cache_path, bytes);
        }

        Ok(raw)
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(&self, url: &str) -> anyhow::Result<T> {
        sleep(RATE_LIMIT_DELAY).await;
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;

        if !resp.status().is_success() {
            bail!("HTTP {} for {url}", resp.status());
        }
        resp.json::<T>()
            .await
            .with_context(|| format!("decoding JSON from {url}"))
    }
}
