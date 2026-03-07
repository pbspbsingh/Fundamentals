//! High-level fetch functions that combine client calls, parsing, and assembly
//! of raw data into the typed structs defined in `types`.

use chrono::{Datelike, NaiveDate, Utc};

use crate::{
    client::EdgarClient,
    compute,
    parse::{self, Fact},
    types::*,
};

/// Top-level entry point. Fetches all EDGAR data for `ticker` and returns
/// a fully populated `EdgarFundamentals`.
pub async fn fetch_fundamentals(
    client: &EdgarClient,
    ticker: &str,
) -> anyhow::Result<EdgarFundamentals> {
    let (cik, company_name) = client.resolve_cik(ticker).await?;

    let facts_json = client.fetch_company_facts(&cik).await?;
    let facts = &facts_json["facts"];

    // ── Earnings ─────────────────────────────────────────────────────────────
    let eps_diluted_facts = parse::extract_quarterly_eps(
        facts,
        &["EarningsPerShareDiluted", "EarningsPerShareBasicAndDiluted"],
    );
    let eps_basic_facts = parse::extract_quarterly_eps(
        facts,
        &["EarningsPerShareBasic", "EarningsPerShareBasicAndDiluted"],
    );
    let net_income_facts = parse::extract_quarterly(
        facts,
        // ProfitLoss is used by some companies instead of NetIncomeLoss (e.g. MNST)
        &[
            "NetIncomeLoss",
            "ProfitLoss",
            "NetIncomeLossAvailableToCommonStockholdersBasic",
        ],
    );
    let diluted_shares_facts = parse::extract_quarterly_shares(
        facts,
        &[
            "WeightedAverageNumberOfDilutedSharesOutstanding",
            // Used by companies where basic = diluted (e.g. BKR)
            "WeightedAverageNumberOfShareOutstandingBasicAndDiluted",
        ],
    );

    // ── Revenue ───────────────────────────────────────────────────────────────
    let revenue_facts = parse::extract_quarterly(
        facts,
        &[
            "Revenues",
            "RevenueFromContractWithCustomerExcludingAssessedTax",
            // Used by some companies (e.g. CRWD, ODFL, KHC)
            "RevenueFromContractWithCustomerIncludingAssessedTax",
            "SalesRevenueNet",
            "SalesRevenueGoodsNet",    // e.g. MNST
            "SalesRevenueServicesNet", // e.g. ODFL
        ],
    );

    // ── Margins ───────────────────────────────────────────────────────────────
    let gross_profit_facts = parse::extract_quarterly(facts, &["GrossProfit"]);
    let cogs_facts =
        parse::extract_quarterly(facts, &["CostOfGoodsAndServicesSold", "CostOfRevenue"]);
    let op_income_facts = parse::extract_quarterly(facts, &["OperatingIncomeLoss"]);

    // ── Cash Flow (annual) ────────────────────────────────────────────────────
    let ocf_facts = parse::extract_annual(
        facts,
        &[
            "NetCashProvidedByUsedInOperatingActivities",
            // Used by some companies (e.g. GEHC, spun-off entities)
            "NetCashProvidedByUsedInOperatingActivitiesContinuingOperations",
        ],
    );
    let capex_facts = parse::extract_annual(facts, &["PaymentsToAcquirePropertyPlantAndEquipment"]);

    // ── Shares ────────────────────────────────────────────────────────────────
    let shares_outstanding_facts =
        parse::extract_quarterly_shares(facts, &["CommonStockSharesOutstanding"]);

    // ── Balance Sheet ─────────────────────────────────────────────────────────
    let cash_facts = parse::extract_quarterly(facts, &["CashAndCashEquivalentsAtCarryingValue"]);
    let lt_debt_facts = parse::extract_quarterly(facts, &["LongTermDebt"]);
    let st_debt_facts =
        parse::extract_quarterly(facts, &["ShortTermBorrowings", "LongTermDebtCurrent"]);
    let equity_facts = parse::extract_quarterly(facts, &["StockholdersEquity"]);

    // ── Assemble typed structs ────────────────────────────────────────────────
    let mut earnings = build_earnings(
        &eps_diluted_facts,
        &eps_basic_facts,
        &net_income_facts,
        &diluted_shares_facts,
        12,
    );
    compute::compute_earnings_growth(&mut earnings);

    let mut revenue = build_revenue(&revenue_facts, 12);
    compute::compute_revenue_growth(&mut revenue);

    let mut margins = build_margins(
        &gross_profit_facts,
        &cogs_facts,
        &op_income_facts,
        &net_income_facts,
        &revenue_facts,
        12,
    );
    compute::compute_margins(&mut margins);

    let mut cash_flows = build_cash_flows(&ocf_facts, &capex_facts, 4);
    compute::compute_cash_flows(&mut cash_flows);

    let mut shares = build_shares(&shares_outstanding_facts, &diluted_shares_facts, 12);
    compute::compute_shares_change(&mut shares);

    let mut balance_sheet = build_balance_sheet(
        &cash_facts,
        &lt_debt_facts,
        &st_debt_facts,
        &equity_facts,
        8,
    );
    compute::compute_balance_sheet(&mut balance_sheet);

    // ROE: net income TTM / avg equity, annual
    let mut roe = build_roe(&net_income_facts, &equity_facts, 4);
    compute::compute_roe(&mut roe, &balance_sheet);

    // ── Insider transactions ──────────────────────────────────────────────────
    let insider_transactions = fetch_insider_transactions(client, &cik).await?;

    Ok(EdgarFundamentals {
        ticker: ticker.to_uppercase(),
        cik,
        company_name,
        fetched_at: Utc::now(),
        earnings,
        revenue,
        margins,
        cash_flows,
        shares,
        balance_sheet,
        roe,
        insider_transactions,
    })
}

// ── Builder helpers ───────────────────────────────────────────────────────────

fn build_earnings(
    eps_d: &[Fact],
    eps_b: &[Fact],
    net_income: &[Fact],
    diluted_sh: &[Fact],
    limit: usize,
) -> Vec<EarningsHistory> {
    // Use EPS dates as anchor if available, else fall back to net_income dates.
    // fp/fy metadata must come from whichever source provides the anchor.
    let (anchor_dates, anchor_facts): (Vec<NaiveDate>, &[Fact]) = if !eps_d.is_empty() {
        (eps_d.iter().map(|f| f.end).collect(), eps_d)
    } else {
        (net_income.iter().map(|f| f.end).collect(), net_income)
    };
    let mut dates = anchor_dates;
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| EarningsHistory {
            period_of_report: d,
            fiscal_quarter: quarter_from_fp(fp_for(anchor_facts, d).as_deref()),
            fiscal_year: fy_for(anchor_facts, d),
            eps_diluted: val_for(eps_d, d),
            eps_basic: val_for(eps_b, d),
            net_income: val_for_i64(net_income, d),
            diluted_shares_outstanding: val_for_i64(diluted_sh, d),
            eps_yoy_growth: None,
            eps_qoq_growth: None,
            eps_acceleration: None,
        })
        .collect()
}

fn build_revenue(revenue: &[Fact], limit: usize) -> Vec<RevenueHistory> {
    let mut dates: Vec<NaiveDate> = revenue.iter().map(|f| f.end).collect();
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| RevenueHistory {
            period_of_report: d,
            fiscal_quarter: quarter_from_fp(fp_for(revenue, d).as_deref()),
            fiscal_year: fy_for(revenue, d),
            revenue: val_for_i64(revenue, d),
            revenue_yoy_growth: None,
            revenue_qoq_growth: None,
        })
        .collect()
}

fn build_margins(
    gross_profit: &[Fact],
    cogs: &[Fact],
    op_income: &[Fact],
    net_income: &[Fact],
    revenue: &[Fact],
    limit: usize,
) -> Vec<MarginHistory> {
    let mut dates: Vec<NaiveDate> = gross_profit
        .iter()
        .chain(op_income.iter())
        .chain(net_income.iter())
        .map(|f| f.end)
        .collect();
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| MarginHistory {
            period_of_report: d,
            gross_profit: val_for_i64(gross_profit, d),
            cost_of_revenue: val_for_i64(cogs, d),
            operating_income: val_for_i64(op_income, d),
            net_income: val_for_i64(net_income, d),
            revenue: val_for_i64(revenue, d),
            gross_margin_pct: None,
            operating_margin_pct: None,
            net_margin_pct: None,
        })
        .collect()
}

fn build_cash_flows(ocf: &[Fact], capex: &[Fact], limit: usize) -> Vec<CashFlowHistory> {
    let mut dates: Vec<NaiveDate> = ocf.iter().chain(capex.iter()).map(|f| f.end).collect();
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| CashFlowHistory {
            period_of_report: d,
            fiscal_year: d.year() as u16,
            operating_cash_flow: val_for_i64(ocf, d),
            capex: val_for_i64(capex, d).map(|v| v.abs()),
            free_cash_flow: None,
        })
        .collect()
}

fn build_shares(outstanding: &[Fact], diluted: &[Fact], limit: usize) -> Vec<SharesHistory> {
    let mut dates: Vec<NaiveDate> = outstanding
        .iter()
        .chain(diluted.iter())
        .map(|f| f.end)
        .collect();
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| SharesHistory {
            period_of_report: d,
            shares_outstanding: val_for_i64(outstanding, d),
            shares_diluted: val_for_i64(diluted, d),
            qoq_change_pct: None,
        })
        .collect()
}

fn build_balance_sheet(
    cash: &[Fact],
    lt_debt: &[Fact],
    st_debt: &[Fact],
    equity: &[Fact],
    limit: usize,
) -> Vec<BalanceSheetHistory> {
    let mut dates: Vec<NaiveDate> = cash.iter().chain(equity.iter()).map(|f| f.end).collect();
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| BalanceSheetHistory {
            period_of_report: d,
            cash: val_for_i64(cash, d),
            long_term_debt: val_for_i64(lt_debt, d),
            short_term_debt: val_for_i64(st_debt, d),
            total_debt: None,
            stockholders_equity: val_for_i64(equity, d),
            debt_to_equity: None,
        })
        .collect()
}

fn build_roe(net_income: &[Fact], _equity: &[Fact], limit: usize) -> Vec<ReturnOnEquityHistory> {
    // Use annual net income dates as anchors
    let mut annual_ni: Vec<Fact> = {
        // Sum 4 quarters of net income per fiscal year as TTM proxy
        // For simplicity, use annual 10-K net income if available, else skip
        net_income.to_vec()
    };
    annual_ni.sort_by_key(|f| f.end);
    // Use last `limit` dates
    let mut dates: Vec<NaiveDate> = annual_ni.iter().map(|f| f.end).collect();
    dates.sort();
    dates.dedup();
    let dates = tail(&dates, limit);

    dates
        .iter()
        .map(|&d| {
            // TTM net income: sum the 4 quarters ending at or before d
            let ttm = sum_ttm(net_income, d);
            ReturnOnEquityHistory {
                period_of_report: d,
                fiscal_year: d.year() as u16,
                net_income_ttm: ttm,
                avg_equity: None, // filled by compute::compute_roe
                roe_pct: None,
            }
        })
        .collect()
}

/// Sum the most recent 4 quarterly values ending on or before `end`.
fn sum_ttm(facts: &[Fact], end: NaiveDate) -> Option<i64> {
    let mut relevant: Vec<&Fact> = facts.iter().filter(|f| f.end <= end).collect();
    relevant.sort_by_key(|f| f.end);

    // Take the 4 most recent entries
    let last4: Vec<&Fact> = relevant.iter().rev().take(4).copied().collect();

    if last4.len() < 4 {
        return None;
    }

    // Validate that all 4 quarters are consecutive — no gap > 120 days between
    // adjacent entries. last4 is in reverse chronological order.
    for i in 0..last4.len() - 1 {
        let gap = (last4[i].end - last4[i + 1].end).num_days().abs();
        if gap > 120 {
            return None;
        }
    }

    Some(last4.iter().map(|f| f.val as i64).sum())
}

// ── Insider transactions ──────────────────────────────────────────────────────

async fn fetch_insider_transactions(
    client: &EdgarClient,
    cik: &str,
) -> anyhow::Result<Vec<InsiderTransaction>> {
    let submissions = client.fetch_submissions(cik).await?;

    let recent = &submissions["filings"]["recent"];
    let empty: Vec<serde_json::Value> = vec![];
    let forms = recent["form"].as_array().unwrap_or(&empty);
    let dates = recent["filingDate"].as_array().unwrap_or(&empty);
    let accns = recent["accessionNumber"].as_array().unwrap_or(&empty);
    let primary_docs = recent["primaryDocument"].as_array().unwrap_or(&empty);

    let cutoff = Utc::now().date_naive() - chrono::Duration::days(365);

    // (filing_date, accession_number, primary_document)
    let mut form4_accns: Vec<(NaiveDate, String, String)> = forms
        .iter()
        .zip(dates.iter())
        .zip(accns.iter())
        .zip(primary_docs.iter())
        .filter_map(|(((f, d), a), p)| {
            if f.as_str()? != "4" {
                return None;
            }
            let date = NaiveDate::parse_from_str(d.as_str()?, "%Y-%m-%d").ok()?;
            if date < cutoff {
                return None;
            }
            Some((date, a.as_str()?.to_string(), p.as_str()?.to_string()))
        })
        .collect();

    form4_accns.sort_by_key(|(d, _, _)| *d);

    let mut all_txs: Vec<InsiderTransaction> = vec![];

    for (_filing_date, accn, primary_doc) in &form4_accns {
        let xml = match client.fetch_form4_xml(cik, accn, primary_doc).await {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Warning: skipping Form 4 {accn}: {e}");
                continue;
            }
        };

        let raw_txs = match parse::parse_form4_xml(&xml) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Warning: failed to parse Form 4 {accn}: {e}");
                continue;
            }
        };

        for tx in raw_txs {
            let is_open_market = tx.code == "P" || tx.code == "S";
            let total_value = tx.price.map(|p| tx.shares * p);
            all_txs.push(InsiderTransaction {
                transaction_date: tx.date,
                insider_name: tx.insider_name,
                insider_role: tx.insider_role,
                is_open_market,
                shares: tx.shares as i64,
                price_per_share: tx.price,
                acquisition_or_disposition: tx.acq_disp,
                total_value,
            });
        }
    }

    all_txs.sort_by_key(|t| t.transaction_date);
    Ok(all_txs)
}

// ── Fact lookup helpers ───────────────────────────────────────────────────────

fn val_for(facts: &[Fact], date: NaiveDate) -> Option<f64> {
    facts.iter().find(|f| f.end == date).map(|f| f.val)
}

fn val_for_i64(facts: &[Fact], date: NaiveDate) -> Option<i64> {
    val_for(facts, date).map(|v| v as i64)
}

fn fp_for(facts: &[Fact], date: NaiveDate) -> Option<String> {
    facts.iter().find(|f| f.end == date).map(|f| f.fp.clone())
}

fn fy_for(facts: &[Fact], date: NaiveDate) -> u16 {
    facts
        .iter()
        .find(|f| f.end == date)
        .map(|f| f.fy)
        .unwrap_or(date.year() as u16)
}

fn quarter_from_fp(fp: Option<&str>) -> u8 {
    match fp {
        Some("Q1") => 1,
        Some("Q2") => 2,
        Some("Q3") => 3,
        Some("Q4") | Some("FY") => 4,
        _ => 0,
    }
}

/// Return the last `n` elements of a sorted, deduped date vec.
fn tail(dates: &[NaiveDate], n: usize) -> Vec<NaiveDate> {
    let skip = dates.len().saturating_sub(n);
    dates[skip..].to_vec()
}
