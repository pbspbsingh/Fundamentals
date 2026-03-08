mod parse;
mod table;
mod utils;

use crate::TV_HOME;
use anyhow::Context;
use chrome_driver::{Browser, Page, Sleepable};
use model::Ticker;
use model::financials::TradingViewFinancials;
use tracing::info;

pub struct FinancialScraper {
    browser: Option<Browser>,
    page: Option<Page>,
}

impl Drop for FinancialScraper {
    fn drop(&mut self) {
        if let Some(browser) = self.browser.take()
            && let Some(mut page) = self.page.take()
        {
            tokio::task::spawn(async move {
                page.close_me().await;
                drop(browser);
            });
        }
    }
}

impl FinancialScraper {
    pub async fn new() -> anyhow::Result<Self> {
        let browser = super::launch_browser().await?;
        let page = browser.new_page(TV_HOME).await?;
        page.wait_for_navigation().await?.sleep().await;
        Ok(Self {
            browser: Some(browser),
            page: Some(page),
        })
    }

    pub async fn fetch_financials(&self, ticker: &Ticker) -> anyhow::Result<TradingViewFinancials> {
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

        info!("Clicking statements tab...");
        self.page()
            .find_element("a#statements")
            .await?
            .click()
            .await?;
        self.page().sleep().await;

        let currency = self.currency().await?;

        info!("\nFetching income Statements...");
        self.switch_tab(true).await?;
        let quarterly_income = self.parse_income_statement(true).await?;
        self.switch_tab(false).await?;
        let annual_income = self.parse_income_statement(false).await?;
        let ttm_income = self.parse_ttm_income().await.map_or_else(
            |e| {
                tracing::warn!("TTM income unavailable: {e:#}");
                None
            },
            Some,
        );

        info!("\nFetching Balance Sheet...");
        self.page()
            .find_element("a[id='balance sheet']")
            .await?
            .click()
            .await?;
        self.page().sleep().await;
        self.switch_tab(true).await?;
        let quarterly_balance_sheet = self.parse_balance_sheet(true).await?;
        self.switch_tab(false).await?;
        let annual_balance_sheet = self.parse_balance_sheet(false).await?;

        info!("\nFetching Cash Flow...");
        self.page()
            .find_element("a[id='cash flow']")
            .await?
            .click()
            .await?;
        self.page().sleep().await;
        self.switch_tab(true).await?;
        let quarterly_cash_flow = self.parse_cash_flow(true).await?;
        self.switch_tab(false).await?;
        let annual_cash_flow = self.parse_cash_flow(false).await?;
        let ttm_cash_flow = self.parse_ttm_cash_flow().await.map_or_else(
            |e| {
                tracing::warn!("TTM cash flow unavailable: {e:#}");
                None
            },
            Some,
        );

        info!("\nFetching Statistics...");
        self.page()
            .find_element("a#statistics")
            .await?
            .click()
            .await?;
        self.page().sleep().await;
        self.switch_tab(true).await?;
        let quarterly_statistics = self.parse_statistics(true).await?;
        self.switch_tab(false).await?;
        let annual_statistics = self.parse_statistics(false).await?;

        Ok(TradingViewFinancials {
            ticker: ticker.ticker.clone(),
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
        })
    }

    fn page(&self) -> &Page {
        self.page.as_ref().unwrap()
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
