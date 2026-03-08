use crate::TV_HOME;
use anyhow::Context;
use chrome_driver::{Browser, Page, Sleepable};
use chrono::{Local, NaiveDate};
use model::Ticker;
use model::statements::{
    BalanceSheetEntry, CashFlowEntry, IncomeStatementEntry, Period, Periodicity,
    TradingViewFinancials,
};
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

        info!("Fetching income Statements...");
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

        info!("Fetching Balance Sheet...");
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

        info!("Fetching Cash Flow...");
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

        Ok(TradingViewFinancials {
            ticker: ticker.ticker.clone(),
            currency: "USD".into(),
            scraped_at: Local::now(),

            quarterly_income,
            annual_income,

            quarterly_balance_sheet,
            annual_balance_sheet,

            quarterly_cash_flow,
            annual_cash_flow,

            ttm_income,
            ttm_cash_flow,
        })
    }

    fn page(&self) -> &Page {
        self.page.as_ref().unwrap()
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

    async fn parse_income_statement(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<IncomeStatementEntry>> {
        let data = self.evaluate_table_js().await?;

        let columns = data["columns"].as_array().with_context(|| {
            format!(
                "JS data missing 'columns' array; top-level keys: {:?}",
                data.as_object().map(|o| o.keys().collect::<Vec<_>>())
            )
        })?;
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
        let columns = data["columns"].as_array().with_context(|| {
            format!(
                "JS data missing 'columns' array; top-level keys: {:?}",
                data.as_object().map(|o| o.keys().collect::<Vec<_>>())
            )
        })?;
        let rows = &data["rows"];

        let i = columns
            .iter()
            .position(|col| col["date"].is_null())
            .with_context(|| {
                let labels: Vec<_> = columns.iter().filter_map(|c| c["label"].as_str()).collect();
                format!(
                    "No TTM column found among {} columns: {labels:?}",
                    columns.len()
                )
            })?;

        if rows["Total revenue"][i]["locked"]
            .as_bool()
            .unwrap_or(false)
        {
            anyhow::bail!("TTM column (index {i} of {}) is paywalled", columns.len());
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
            period: Period {
                period_end,
                periodicity: Periodicity::Annual,
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
    }

    async fn parse_balance_sheet(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<BalanceSheetEntry>> {
        let data = self.evaluate_table_js().await?;
        let columns = data["columns"].as_array().with_context(|| {
            format!(
                "JS data missing 'columns' array; top-level keys: {:?}",
                data.as_object().map(|o| o.keys().collect::<Vec<_>>())
            )
        })?;
        let rows = &data["rows"];
        let periodicity = if is_quarterly {
            Periodicity::Quarterly
        } else {
            Periodicity::Annual
        };

        let mut entries: Vec<BalanceSheetEntry> = columns
            .iter()
            .enumerate()
            .filter_map(|(i, col)| {
                let date = parse_month_year(col["date"].as_str()?)?;
                if rows["Total assets"][i]["locked"].as_bool().unwrap_or(false) {
                    return None;
                }
                let v = |name: &str| parse_value(rows[name][i]["value"].as_str().unwrap_or(""));
                let c = |name: &str| parse_pct(rows[name][i]["change"].as_str().unwrap_or(""));
                Some(BalanceSheetEntry {
                    period: Period {
                        period_end: date,
                        periodicity,
                    },
                    total_assets: v("Total assets"),
                    total_assets_yoy: c("Total assets"),
                    total_liabilities: v("Total liabilities"),
                    total_liabilities_yoy: c("Total liabilities"),
                    total_equity: v("Total equity"),
                    total_equity_yoy: c("Total equity"),
                    total_liabilities_and_equity: v("Total liabilities & shareholders' equities"),
                    total_debt: v("Total debt"),
                    net_debt: v("Net debt"),
                })
            })
            .collect();

        entries.sort_by_key(|e| e.period.period_end);
        Ok(entries)
    }

    async fn parse_cash_flow(&self, is_quarterly: bool) -> anyhow::Result<Vec<CashFlowEntry>> {
        let data = self.evaluate_table_js().await?;
        let columns = data["columns"].as_array().with_context(|| {
            format!(
                "JS data missing 'columns' array; top-level keys: {:?}",
                data.as_object().map(|o| o.keys().collect::<Vec<_>>())
            )
        })?;
        let rows = &data["rows"];
        let periodicity = if is_quarterly {
            Periodicity::Quarterly
        } else {
            Periodicity::Annual
        };

        let mut entries: Vec<CashFlowEntry> = columns
            .iter()
            .enumerate()
            .filter_map(|(i, col)| {
                let date = parse_month_year(col["date"].as_str()?)?;
                if rows["Cash from operating activities"][i]["locked"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    return None;
                }
                let v = |name: &str| parse_value(rows[name][i]["value"].as_str().unwrap_or(""));
                let c = |name: &str| parse_pct(rows[name][i]["change"].as_str().unwrap_or(""));
                Some(CashFlowEntry {
                    period: Period {
                        period_end: date,
                        periodicity,
                    },
                    operating_cash_flow: v("Cash from operating activities"),
                    operating_cash_flow_yoy: c("Cash from operating activities"),
                    investing_cash_flow: v("Cash from investing activities"),
                    investing_cash_flow_yoy: c("Cash from investing activities"),
                    financing_cash_flow: v("Cash from financing activities"),
                    financing_cash_flow_yoy: c("Cash from financing activities"),
                    free_cash_flow: v("Free cash flow"),
                    free_cash_flow_yoy: c("Free cash flow"),
                })
            })
            .collect();

        entries.sort_by_key(|e| e.period.period_end);
        Ok(entries)
    }

    async fn parse_ttm_cash_flow(&self) -> anyhow::Result<CashFlowEntry> {
        let data = self.evaluate_table_js().await?;
        let columns = data["columns"].as_array().with_context(|| {
            format!(
                "JS data missing 'columns' array; top-level keys: {:?}",
                data.as_object().map(|o| o.keys().collect::<Vec<_>>())
            )
        })?;
        let rows = &data["rows"];

        let i = columns
            .iter()
            .position(|col| col["date"].is_null())
            .with_context(|| {
                let labels: Vec<_> = columns.iter().filter_map(|c| c["label"].as_str()).collect();
                format!(
                    "No TTM column found among {} columns: {labels:?}",
                    columns.len()
                )
            })?;

        if rows["Cash from operating activities"][i]["locked"]
            .as_bool()
            .unwrap_or(false)
        {
            anyhow::bail!("TTM column (index {i} of {}) is paywalled", columns.len());
        }

        let period_end = columns
            .iter()
            .filter_map(|col| parse_month_year(col["date"].as_str()?))
            .max()
            .unwrap_or_else(|| Local::now().date_naive());

        let v = |name: &str| parse_value(rows[name][i]["value"].as_str().unwrap_or(""));
        let c = |name: &str| parse_pct(rows[name][i]["change"].as_str().unwrap_or(""));
        Ok(CashFlowEntry {
            period: Period {
                period_end,
                periodicity: Periodicity::Annual,
            },
            operating_cash_flow: v("Cash from operating activities"),
            operating_cash_flow_yoy: c("Cash from operating activities"),
            investing_cash_flow: v("Cash from investing activities"),
            investing_cash_flow_yoy: c("Cash from investing activities"),
            financing_cash_flow: v("Cash from financing activities"),
            financing_cash_flow_yoy: c("Cash from financing activities"),
            free_cash_flow: v("Free cash flow"),
            free_cash_flow_yoy: c("Free cash flow"),
        })
    }
}

// ── Value parsing ─────────────────────────────────────────────────────────────

#[inline]
fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Parse a TradingView numeric value like "634.34 M", "1.63 B", "-105.53 M".
/// Unicode directional marks (U+202A/202C) are stripped; U+2212 is treated as minus.
/// Returns `None` for TradingView's "no data" sentinel "—" (U+2014) and empty strings.
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
    if clean.is_empty() || clean == "\u{2014}" {
        return None;
    }

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
    Some(round3(if neg { -(n * mult) } else { n * mult }))
}

/// Parse a TradingView percentage string like "+20.78%" or "−15.40%" → fractional f64.
/// Returns `None` for TradingView's "no data" sentinel "—" (U+2014) and empty strings.
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
    let clean = clean.trim();
    if clean.is_empty() || clean == "\u{2014}" {
        return None;
    }
    // Replace unicode minus sign with ASCII minus, then strip trailing '%'
    let clean = clean.replace('\u{2212}', "-");
    let n: f64 = clean.trim_end_matches('%').parse().ok()?;
    Some(round3(n / 100.0))
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
