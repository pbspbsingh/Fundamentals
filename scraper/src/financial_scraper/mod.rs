mod parse;
mod table;
mod utils;

use crate::TV_HOME;
use anyhow::Context;
use chrome_driver::{Page, Sleepable};
use model::FetchConfig;
use model::Ticker;
use model::financials::TradingViewFinancials;
use std::sync::Arc;
use tracing::info;

pub struct FinancialScraper {
    page: Arc<Page>,
}

impl FinancialScraper {
    pub async fn new(page: Arc<Page>) -> Self {
        Self { page }
    }

    /// Fetch all financial sections (legacy: used by standalone CLI runner).
    pub async fn fetch_financials(&self, ticker: &Ticker) -> anyhow::Result<TradingViewFinancials> {
        self.fetch_financials_with_config(ticker, &FetchConfig::default()).await
    }

    /// Fetch only the sections specified in `config`.
    pub async fn fetch_financials_with_config(
        &self,
        ticker: &Ticker,
        config: &FetchConfig,
    ) -> anyhow::Result<TradingViewFinancials> {
        let needs_page = config.income_statement
            || config.balance_sheet
            || config.cash_flow
            || config.statistics
            || config.earnings;

        if !needs_page {
            // Nothing to scrape from TradingView financials page.
            return Ok(TradingViewFinancials {
                currency: String::new(),
                quarterly_income: vec![],
                annual_income: vec![],
                quarterly_balance_sheet: vec![],
                annual_balance_sheet: vec![],
                quarterly_cash_flow: vec![],
                annual_cash_flow: vec![],
                ttm_income: None,
                ttm_cash_flow: None,
                quarterly_statistics: vec![],
                annual_statistics: vec![],
                quarterly_earnings: vec![],
                annual_earnings: vec![],
            });
        }

        info!("Navigating to financials page of {ticker}...");
        self.page()
            .goto(format!(
                "{TV_HOME}/symbols/{}-{}/financials-overview/",
                ticker.exchange, ticker.ticker
            ))
            .await?
            .wait_for_navigation()
            .await?
            .sleep()
            .await;

        let needs_statements =
            config.income_statement || config.balance_sheet || config.cash_flow || config.statistics;

        let mut currency = String::new();
        let mut quarterly_income = vec![];
        let mut annual_income = vec![];
        let mut ttm_income = None;
        let mut quarterly_balance_sheet = vec![];
        let mut annual_balance_sheet = vec![];
        let mut quarterly_cash_flow = vec![];
        let mut annual_cash_flow = vec![];
        let mut ttm_cash_flow = None;
        let mut quarterly_statistics = vec![];
        let mut annual_statistics = vec![];
        let mut quarterly_earnings = vec![];
        let mut annual_earnings = vec![];

        if needs_statements {
            info!("Clicking statements tab...");
            self.page()
                .find_element("a#statements")
                .await?
                .click()
                .await?;
            self.page().sleep().await;

            currency = self.currency().await?;

            if config.income_statement {
                info!("\nFetching Income Statements...");
                self.switch_tab(true).await?;
                quarterly_income = self.parse_income_statement(true).await?;
                self.switch_tab(false).await?;
                annual_income = self.parse_income_statement(false).await?;
                ttm_income = self.parse_ttm_income().await.map_or_else(
                    |e| {
                        tracing::warn!("TTM income unavailable: {e:#}");
                        None
                    },
                    Some,
                );
            }

            if config.balance_sheet {
                info!("\nFetching Balance Sheet...");
                self.page()
                    .find_element("a[id='balance sheet']")
                    .await?
                    .click()
                    .await?;
                self.page().sleep().await;
                self.switch_tab(true).await?;
                quarterly_balance_sheet = self.parse_balance_sheet(true).await?;
                self.switch_tab(false).await?;
                annual_balance_sheet = self.parse_balance_sheet(false).await?;
            }

            if config.cash_flow {
                info!("\nFetching Cash Flow...");
                self.page()
                    .find_element("a[id='cash flow']")
                    .await?
                    .click()
                    .await?;
                self.page().sleep().await;
                self.switch_tab(true).await?;
                quarterly_cash_flow = self.parse_cash_flow(true).await?;
                self.switch_tab(false).await?;
                annual_cash_flow = self.parse_cash_flow(false).await?;
                ttm_cash_flow = self.parse_ttm_cash_flow().await.map_or_else(
                    |e| {
                        tracing::warn!("TTM cash flow unavailable: {e:#}");
                        None
                    },
                    Some,
                );
            }

            if config.statistics {
                info!("\nFetching Statistics...");
                self.page()
                    .find_element("a#statistics")
                    .await?
                    .click()
                    .await?;
                self.page().sleep().await;
                self.switch_tab(true).await?;
                quarterly_statistics = self.parse_statistics(true).await?;
                self.switch_tab(false).await?;
                annual_statistics = self.parse_statistics(false).await?;
            }
        }

        if config.earnings {
            info!("\nFetching Earnings...");
            self.page()
                .find_element("a#earnings")
                .await?
                .click()
                .await?;
            self.page().sleep().await;
            quarterly_earnings = self.parse_earnings(true).await?;
            annual_earnings = self.parse_earnings(false).await?;
        }

        Ok(TradingViewFinancials {
            currency,
            quarterly_income,
            annual_income,
            quarterly_balance_sheet,
            annual_balance_sheet,
            quarterly_cash_flow,
            annual_cash_flow,
            ttm_income,
            ttm_cash_flow,
            quarterly_statistics,
            annual_statistics,
            quarterly_earnings,
            annual_earnings,
        })
    }

    fn page(&self) -> &Page {
        self.page.as_ref()
    }

    async fn currency(&self) -> anyhow::Result<String> {
        let div = self
            .page()
            .find_xpath("//div[starts-with(@class, 'filling') and contains(., 'Currency:')]")
            .await?;
        Ok(div
            .inner_text()
            .await?
            .context("No currency text found")?
            .trim_start_matches("Currency:")
            .trim()
            .to_uppercase())
    }

    async fn switch_tab(&self, to_quarterly: bool) -> anyhow::Result<()> {
        let text = if to_quarterly {
            info!("Switching to Quarterly tab");
            "Quarterly"
        } else {
            info!("Switching to Annual tab");
            "Annual"
        };
        self.page()
            .find_xpath(format!(
                r#"//span[contains(@class, "tabContent") and .="{text}"]"#
            ))
            .await?
            .click()
            .await?;
        self.page().sleep().await;
        Ok(())
    }

    async fn evaluate_table_js(&self) -> anyhow::Result<serde_json::Value> {
        const JS: &str = r#"(function() {
            let headerEl = null;
            for (const el of document.querySelectorAll('[class*="values-"]')) {
                if (!el.closest('[data-name]') && el.querySelector('[class*="subvalue-"]')) {
                    headerEl = el;
                    break;
                }
            }

            const columns = [];
            if (headerEl) {
                for (const cell of headerEl.children) {
                    const val = cell.querySelector('[class*="value-"]:not([class*="subvalue-"])');
                    const sub = cell.querySelector('[class*="subvalue-"]');
                    columns.push({
                        label: val ? val.textContent.trim() : null,
                        date:  sub ? sub.textContent.trim() : null
                    });
                }
            }

            const rows = {};
            for (const row of document.querySelectorAll('[data-name]')) {
                const name = row.getAttribute('data-name');
                let valuesEl = null;
                for (const el of row.querySelectorAll('[class*="values-"]')) {
                    if (el.children.length > 0) { valuesEl = el; break; }
                }
                if (!valuesEl) continue;

                const cells = [];
                for (const cell of valuesEl.children) {
                    const val = cell.querySelector('[class*="value-"]:not([class*="subvalue-"])');
                    const chg = cell.querySelector('[class*="change-"]');
                    cells.push({
                        value:  val ? val.textContent.trim() : null,
                        change: chg ? chg.textContent.trim() : null,
                        locked: !!cell.querySelector('[class*="lockButton"]')
                    });
                }
                rows[name] = cells;
            }

            return { columns, rows };
        })()"#;

        let result = self.page().evaluate(JS).await?;
        let raw = result.value().cloned();
        result.into_value().with_context(|| {
            format!(
                "JS table extractor returned a value that could not be deserialized; \
                 raw CDP value: {raw:?}"
            )
        })
    }

    /// Switch both the EPS and Revenue earnings tabs simultaneously.
    /// The earnings page renders two independent tab bars both using id="FY"/"FQ",
    /// so we click them all via JS to avoid partial state.
    pub(super) async fn switch_earnings_tab(&self, to_quarterly: bool) -> anyhow::Result<()> {
        let id = if to_quarterly { "FQ" } else { "FY" };
        info!(
            "Switching earnings tabs to {}",
            if to_quarterly { "Quarterly" } else { "Annual" }
        );
        self.page()
            .evaluate(format!(
                r#"document.querySelectorAll('[id="{id}"]').forEach(b => b.click())"#
            ))
            .await?;
        self.page().sleep().await;
        Ok(())
    }

    pub(super) async fn evaluate_earnings_js(&self) -> anyhow::Result<serde_json::Value> {
        const JS: &str = r#"(function() {
            const tables = document.querySelectorAll('[class*="table-GQWAi9kx"]');
            const result = {};
            const tableNames = ['eps', 'revenue'];

            tables.forEach((tbl, idx) => {
                const key = tableNames[idx] ?? `table${idx}`;

                // Column labels: header row uses container-OWKkVLyj → values-OWKkVLyj
                const headerContainer = tbl.querySelector('[class*="container-OWKkVLyj"]');
                const valuesEl = headerContainer
                    ? headerContainer.querySelector('[class*="values-OWKkVLyj"]')
                    : null;
                const labels = [];
                if (valuesEl) {
                    for (const cell of valuesEl.children) {
                        const val = cell.querySelector('[class*="value-OxVAcLqi"]');
                        labels.push(val ? val.textContent.trim() : null);
                    }
                }

                // Data rows: container-C9MdAMrq with titleText-C9MdAMrq label
                const rows = {};
                for (const row of tbl.querySelectorAll('[class*="container-C9MdAMrq"]')) {
                    const titleEl = row.querySelector('[class*="titleText-C9MdAMrq"]');
                    if (!titleEl) continue;
                    const title = titleEl.textContent.trim();

                    const valuesDiv = row.querySelector('[class*="values-C9MdAMrq"]');
                    const cells = [];
                    if (valuesDiv) {
                        for (const cell of valuesDiv.children) {
                            const val = cell.querySelector('[class*="value-OxVAcLqi"]');
                            const locked = !!cell.querySelector('[class*="lockButton"]');
                            cells.push({ value: val ? val.textContent.trim() : null, locked });
                        }
                    }
                    rows[title] = cells;
                }

                result[key] = { labels, rows };
            });

            return result;
        })()"#;

        let result = self.page().evaluate(JS).await?;
        let raw = result.value().cloned();
        result.into_value().with_context(|| {
            format!("Earnings JS extractor returned a non-deserializable value; raw: {raw:?}")
        })
    }

    async fn fetch_table(&self) -> anyhow::Result<(Vec<serde_json::Value>, serde_json::Value)> {
        let data = self.evaluate_table_js().await?;
        let columns = data["columns"]
            .as_array()
            .with_context(|| {
                format!(
                    "JS data missing 'columns' array; top-level keys: {:?}",
                    data.as_object().map(|o| o.keys().collect::<Vec<_>>())
                )
            })?
            .clone();
        let rows = data["rows"].clone();
        Ok((columns, rows))
    }
}
