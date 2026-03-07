use crate::TV_HOME;
use anyhow::Context;
use chrome_driver::{Browser, Page, Sleepable};
use chrono::{Local, NaiveDate};
use model::statements::{IncomeStatementEntry, Period, Periodicity};
use model::{Ticker, TradingViewFinancials};
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

        let about = self.about().await?;

        info!("Clicking statements tab...");
        self.page()
            .find_element("a#statements")
            .await?
            .click()
            .await?;
        self.page().sleep().await;

        info!("Fetching income statements...");
        self.switch_tab(true).await?;
        let quarterly_income = self.parse_income_statement(true).await?;
        self.switch_tab(false).await?;
        let annual_income = self.parse_income_statement(false).await?;

        info!("Parsing TTM Income...");
        let ttm_income = self.parse_ttm_income().await?;
        eprintln!("{ttm_income:#?}");

        Ok(TradingViewFinancials {
            ticker: ticker.ticker.clone(),
            currency: "USD".into(),
            about,
            scraped_at: Local::now(),

            quarterly_income,
            annual_income,

            quarterly_balance_sheet: vec![],
            annual_balance_sheet: vec![],

            quarterly_cash_flow: vec![],
            annual_cash_flow: vec![],

            ttm_income: Some(ttm_income),
            ttm_cash_flow: None,
        })
    }

    fn page(&self) -> &Page {
        self.page.as_ref().unwrap()
    }

    async fn about(&self) -> anyhow::Result<String> {
        Ok(self
            .page()
            .find_xpath("//p[starts-with(@class, 'description')]")
            .await?
            .inner_text()
            .await?
            .unwrap_or_default())
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

        self.page()
            .evaluate(JS)
            .await?
            .into_value()
            .context("Failed to deserialize table DOM data")
    }

    async fn parse_income_statement(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<IncomeStatementEntry>> {
        let data = self.evaluate_table_js().await?;

        let columns = data["columns"]
            .as_array()
            .context("Missing columns array")?;
        let rows = &data["rows"];
        let periodicity = if is_quarterly {
            Periodicity::Quarterly
        } else {
            Periodicity::Annual
        };

        let mut entries: Vec<IncomeStatementEntry> = columns
            .iter()
            .enumerate()
            .filter_map(|(i, col)| {
                let date = parse_month_year(col["date"].as_str()?)?; // skips TTM (no date)
                // Skip columns where the data is paywalled.
                if rows["Total revenue"][i]["locked"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    return None;
                }
                let v = |name: &str| parse_value(rows[name][i]["value"].as_str().unwrap_or(""));
                let c = |name: &str| parse_pct(rows[name][i]["change"].as_str().unwrap_or(""));
                Some(IncomeStatementEntry {
                    period: Period {
                        period_end: date,
                        periodicity,
                    },
                    total_revenue: v("Total revenue"),
                    total_revenue_yoy: c("Total revenue"),
                    cost_of_goods_sold: v("Cost of goods sold"),
                    gross_profit: v("Gross profit"),
                    operating_expenses_excl_cogs: v("Operating expenses (excl. COGS)"),
                    operating_income: v("Operating income"),
                    operating_income_yoy: c("Operating income"),
                    non_operating_income: v("Non-operating income (total)"),
                    pretax_income: v("Pretax income"),
                    pretax_income_yoy: c("Pretax income"),
                    equity_in_earnings: v("Equity in earnings"),
                    taxes: v("Taxes"),
                    minority_interest: v("Non-controlling/minority interest"),
                    after_tax_other_income: v("After tax other income/expense"),
                    net_income_before_discontinued: v("Net income before discontinued operations"),
                    discontinued_operations: v("Discontinued operations"),
                    net_income: v("Net income"),
                    net_income_yoy: c("Net income"),
                    dilution_adjustment: v("Dilution adjustment"),
                    preferred_dividends: v("Preferred dividends"),
                    net_income_available_to_common: v(
                        "Diluted net income available to common stockholders",
                    ),
                    eps_basic: v("Basic earnings per share (basic EPS)"),
                    eps_basic_yoy: c("Basic earnings per share (basic EPS)"),
                    eps_diluted: v("Diluted earnings per share (diluted EPS)"),
                    eps_diluted_yoy: c("Diluted earnings per share (diluted EPS)"),
                    shares_basic: v("Average basic shares outstanding"),
                    shares_diluted: v("Diluted shares outstanding"),
                    ebitda: v("EBITDA"),
                    ebit: v("EBIT"),
                    ebit_yoy: c("EBIT"),
                    total_operating_expenses: v("Total operating expenses"),
                })
            })
            .collect();

        entries.sort_by_key(|e| e.period.period_end);
        Ok(entries)
    }

    async fn parse_ttm_income(&self) -> anyhow::Result<IncomeStatementEntry> {
        let data = self.evaluate_table_js().await?;
        let columns = data["columns"].as_array().context("Missing columns array")?;
        let rows = &data["rows"];

        let i = columns
            .iter()
            .position(|col| col["date"].is_null())
            .context("TTM column not found")?;

        if rows["Total revenue"][i]["locked"].as_bool().unwrap_or(false) {
            anyhow::bail!("TTM column is paywalled");
        }

        // period_end = end of the most recent dated quarter in the table
        let period_end = columns
            .iter()
            .filter_map(|col| parse_month_year(col["date"].as_str()?))
            .max()
            .unwrap_or_else(|| Local::now().date_naive());

        let v = |name: &str| parse_value(rows[name][i]["value"].as_str().unwrap_or(""));
        let c = |name: &str| parse_pct(rows[name][i]["change"].as_str().unwrap_or(""));
        Ok(IncomeStatementEntry {
            period: Period { period_end, periodicity: Periodicity::Annual },
            total_revenue:                  v("Total revenue"),
            total_revenue_yoy:              c("Total revenue"),
            cost_of_goods_sold:             v("Cost of goods sold"),
            gross_profit:                   v("Gross profit"),
            operating_expenses_excl_cogs:   v("Operating expenses (excl. COGS)"),
            operating_income:               v("Operating income"),
            operating_income_yoy:           c("Operating income"),
            non_operating_income:           v("Non-operating income (total)"),
            pretax_income:                  v("Pretax income"),
            pretax_income_yoy:              c("Pretax income"),
            equity_in_earnings:             v("Equity in earnings"),
            taxes:                          v("Taxes"),
            minority_interest:              v("Non-controlling/minority interest"),
            after_tax_other_income:         v("After tax other income/expense"),
            net_income_before_discontinued: v("Net income before discontinued operations"),
            discontinued_operations:        v("Discontinued operations"),
            net_income:                     v("Net income"),
            net_income_yoy:                 c("Net income"),
            dilution_adjustment:            v("Dilution adjustment"),
            preferred_dividends:            v("Preferred dividends"),
            net_income_available_to_common: v("Diluted net income available to common stockholders"),
            eps_basic:                      v("Basic earnings per share (basic EPS)"),
            eps_basic_yoy:                  c("Basic earnings per share (basic EPS)"),
            eps_diluted:                    v("Diluted earnings per share (diluted EPS)"),
            eps_diluted_yoy:                c("Diluted earnings per share (diluted EPS)"),
            shares_basic:                   v("Average basic shares outstanding"),
            shares_diluted:                 v("Diluted shares outstanding"),
            ebitda:                         v("EBITDA"),
            ebit:                           v("EBIT"),
            ebit_yoy:                       c("EBIT"),
            total_operating_expenses:       v("Total operating expenses"),
        })
    }
}

// ── Value parsing ─────────────────────────────────────────────────────────────

/// Parse a TradingView numeric value like "634.34 M", "1.63 B", "-105.53 M".
/// Unicode directional marks (U+202A/202C) are stripped; U+2212 is treated as minus.
fn parse_value(s: &str) -> Option<f64> {
    let clean: String = s
        .chars()
        .filter(|&c| {
            !matches!(
                c,
                '\u{202A}' | '\u{202B}' | '\u{202C}' | '\u{200E}' | '\u{200F}'
            )
        })
        .collect();
    let clean = clean.trim();

    let (neg, rest) = if clean.starts_with('\u{2212}') || clean.starts_with('-') {
        let skip = clean.chars().next()?.len_utf8();
        (true, &clean[skip..])
    } else {
        (false, clean)
    };

    // Detect suffix by the last character; trim_end() removes whatever separator
    // precedes it (may be U+00A0 non-breaking space, not a regular ASCII space).
    let (num_str, mult) = match rest.chars().last() {
        Some('T') => (rest[..rest.len() - 'T'.len_utf8()].trim_end(), 1e12_f64),
        Some('B') => (rest[..rest.len() - 'B'.len_utf8()].trim_end(), 1e9_f64),
        Some('M') => (rest[..rest.len() - 'M'.len_utf8()].trim_end(), 1e6_f64),
        Some('K') => (rest[..rest.len() - 'K'.len_utf8()].trim_end(), 1e3_f64),
        _ => (rest, 1.0_f64),
    };

    let n: f64 = num_str.trim().parse().ok()?;
    Some(if neg { -(n * mult) } else { n * mult })
}

/// Parse a TradingView percentage string like "+20.78%" or "−15.40%" → fractional f64.
fn parse_pct(s: &str) -> Option<f64> {
    let clean: String = s
        .chars()
        .filter(|&c| {
            !matches!(
                c,
                '\u{202A}' | '\u{202B}' | '\u{202C}' | '\u{200E}' | '\u{200F}'
            )
        })
        .collect();
    // Replace unicode minus sign with ASCII minus, then strip trailing '%'
    let clean = clean.trim().replace('\u{2212}', "-");
    let n: f64 = clean.trim_end_matches('%').parse().ok()?;
    Some(n / 100.0)
}

/// Parse "Mar 2019" → last day of that month (2019-03-31).
fn parse_month_year(s: &str) -> Option<NaiveDate> {
    let mut parts = s.split_whitespace();
    let month: u32 = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i32 = parts.next()?.parse().ok()?;
    // First day of next month minus one day = last day of this month
    let first_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    first_next.pred_opt()
}
