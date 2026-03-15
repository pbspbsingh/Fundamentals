use anyhow::Context;
use chrome_driver::{Page, Sleepable};
use model::Ticker;
use std::sync::Arc;
use tracing::info;

pub struct ScreenerScraper {
    page: Arc<Page>,
}

impl ScreenerScraper {
    pub async fn new(page: Arc<Page>) -> Self {
        Self { page }
    }

    /// Navigate to a TradingView screener URL and extract all visible tickers.
    /// Returns only the `exchange` and `ticker` fields; all other data is sourced elsewhere.
    pub async fn fetch_tickers(&self, screener_url: &str) -> anyhow::Result<Vec<Ticker>> {
        info!("Navigating to TradingView screener: {screener_url}");
        self.page
            .goto(screener_url)
            .await?
            .wait_for_navigation()
            .await?
            .sleep()
            .await;

        // Wait a bit more for the screener table to fully render
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Extract tickers via JS.
        // TradingView screener rows contain links like /symbols/NASDAQ-AAPL/ or
        // data attributes with the full symbol like "NASDAQ:AAPL".
        // We look for both patterns and prefer the link href pattern.
        const JS: &str = r#"(function() {
            const tickers = [];
            const seen = new Set();

            // Pattern 1: links in the format /symbols/EXCHANGE-TICKER/
            const links = document.querySelectorAll('a[href*="/symbols/"]');
            for (const a of links) {
                const match = a.href.match(/\/symbols\/([A-Z0-9]+)-([A-Z0-9.]+)\//);
                if (match) {
                    const key = match[1] + ':' + match[2];
                    if (!seen.has(key)) {
                        seen.add(key);
                        tickers.push({ exchange: match[1], ticker: match[2] });
                    }
                }
            }

            // Pattern 2: data-symbol attributes like "NASDAQ:AAPL"
            if (tickers.length === 0) {
                const cells = document.querySelectorAll('[data-symbol]');
                for (const cell of cells) {
                    const sym = cell.getAttribute('data-symbol');
                    if (sym && sym.includes(':')) {
                        const [exchange, ticker] = sym.split(':');
                        const key = exchange + ':' + ticker;
                        if (!seen.has(key)) {
                            seen.add(key);
                            tickers.push({ exchange, ticker });
                        }
                    }
                }
            }

            // Pattern 3: look for ticker cells in the screener table rows
            if (tickers.length === 0) {
                // TradingView screener table body rows
                const rows = document.querySelectorAll('tr.tv-data-table__row, [class*="row-"][class*="screener"]');
                for (const row of rows) {
                    const symbolEl = row.querySelector('[class*="symbol"], [data-field-key="name"]');
                    if (symbolEl) {
                        const text = symbolEl.textContent.trim();
                        // Might be "AAPL" alone — exchange unclear, default to empty
                        if (text && !seen.has(text)) {
                            seen.add(text);
                            // Try to get exchange from a nearby element
                            const exchEl = row.querySelector('[class*="exchange"], [data-field-key="exchange"]');
                            const exchange = exchEl ? exchEl.textContent.trim() : '';
                            tickers.push({ exchange, ticker: text });
                        }
                    }
                }
            }

            return tickers;
        })()"#;

        let result = self
            .page
            .evaluate(JS)
            .await?;
        let raw = result.value().cloned();
        let json: serde_json::Value = result.into_value().with_context(|| {
            format!(
                "Screener JS extractor returned non-deserializable value; raw: {raw:?}"
            )
        })?;

        let arr = json
            .as_array()
            .with_context(|| "Screener JS did not return an array")?;

        let mut tickers = Vec::new();
        for item in arr {
            let exchange = item["exchange"].as_str().unwrap_or("").to_uppercase();
            let ticker = item["ticker"].as_str().unwrap_or("").to_uppercase();
            if !ticker.is_empty() {
                tickers.push(Ticker::new(exchange, ticker));
            }
        }

        info!("Screener returned {} tickers", tickers.len());
        Ok(tickers)
    }
}
