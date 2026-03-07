//! Derived field calculations applied after raw data is collected and sorted.

use chrono::{Datelike, NaiveDate};

use crate::types::{
    BalanceSheetHistory, CashFlowHistory, EarningsHistory, MarginHistory, ReturnOnEquityHistory,
    RevenueHistory, SharesHistory,
};

/// Compute EPS YoY growth, QoQ growth, and acceleration in-place.
/// Expects `earnings` sorted ascending by period_of_report.
pub fn compute_earnings_growth(earnings: &mut Vec<EarningsHistory>) {
    // Fill missing EPS from net_income / diluted_shares when XBRL EPS is unavailable
    for e in earnings.iter_mut() {
        if e.eps_diluted.is_none() {
            e.eps_diluted = match (e.net_income, e.diluted_shares_outstanding) {
                (Some(ni), Some(sh)) if sh != 0 => Some(round3(ni as f64 / sh as f64)),
                _ => None,
            };
        }
    }

    let n = earnings.len();
    let dates: Vec<NaiveDate> = earnings.iter().map(|e| e.period_of_report).collect();
    let eps_vals: Vec<Option<f64>> = earnings.iter().map(|e| e.eps_diluted).collect();

    for i in 0..n {
        let yoy_eps = find_yoy_index(&dates, dates[i])
            .filter(|&j| j != i)
            .and_then(|j| eps_vals[j]);
        earnings[i].eps_yoy_growth = yoy_growth(eps_vals[i], yoy_eps);
        earnings[i].eps_qoq_growth =
            pct_change(eps_vals[i], if i >= 1 { eps_vals[i - 1] } else { None });
    }
    // Second pass: acceleration = YoY[i] - YoY[i-1]
    let yoy: Vec<Option<f64>> = earnings.iter().map(|e| e.eps_yoy_growth).collect();
    for i in 0..n {
        earnings[i].eps_acceleration = match (yoy[i], if i >= 1 { yoy[i - 1] } else { None }) {
            (Some(a), Some(b)) => Some(round3(a - b)),
            _ => None,
        };
    }
}

/// Compute revenue YoY and QoQ growth in-place.
/// Expects ascending order.
pub fn compute_revenue_growth(revenue: &mut Vec<RevenueHistory>) {
    let n = revenue.len();
    let dates: Vec<NaiveDate> = revenue.iter().map(|r| r.period_of_report).collect();
    let rev_vals: Vec<Option<i64>> = revenue.iter().map(|r| r.revenue).collect();

    for i in 0..n {
        let yoy_val = find_yoy_index(&dates, dates[i])
            .filter(|&j| j != i)
            .and_then(|j| rev_vals[j]);
        revenue[i].revenue_yoy_growth = yoy_growth_i64(rev_vals[i], yoy_val);
        revenue[i].revenue_qoq_growth =
            pct_change_i64(rev_vals[i], if i >= 1 { rev_vals[i - 1] } else { None });
    }
}

/// Compute margin percentages in-place.
pub fn compute_margins(margins: &mut Vec<MarginHistory>) {
    for m in margins.iter_mut() {
        let rev = m.revenue.map(|v| v as f64);
        m.gross_margin_pct = pct_of(m.gross_profit.map(|v| v as f64), rev);
        m.operating_margin_pct = pct_of(m.operating_income.map(|v| v as f64), rev);
        m.net_margin_pct = pct_of(m.net_income.map(|v| v as f64), rev);
    }
}

/// Compute free cash flow in-place.
pub fn compute_cash_flows(cash_flows: &mut Vec<CashFlowHistory>) {
    for cf in cash_flows.iter_mut() {
        cf.free_cash_flow = match (cf.operating_cash_flow, cf.capex) {
            (Some(ocf), Some(capex)) => Some(ocf - capex.abs()),
            (Some(ocf), None) => Some(ocf),
            _ => None,
        };
    }
}

/// Compute QoQ share change in-place.
pub fn compute_shares_change(shares: &mut Vec<SharesHistory>) {
    let n = shares.len();
    let diluted: Vec<Option<i64>> = shares.iter().map(|s| s.shares_diluted).collect();
    for i in 0..n {
        shares[i].qoq_change_pct =
            pct_change_i64(diluted[i], if i >= 1 { diluted[i - 1] } else { None });
    }
}

/// Compute total_debt and debt_to_equity in-place.
pub fn compute_balance_sheet(bs: &mut Vec<BalanceSheetHistory>) {
    for b in bs.iter_mut() {
        b.total_debt = match (b.long_term_debt, b.short_term_debt) {
            (Some(l), Some(s)) => Some(l + s),
            (Some(l), None) => Some(l),
            (None, Some(s)) => Some(s),
            _ => None,
        };
        b.debt_to_equity = match (b.total_debt, b.stockholders_equity) {
            (Some(d), Some(e)) if e != 0 => Some(round3(d as f64 / e as f64)),
            _ => None,
        };
    }
}

/// Compute ROE in-place.
/// Expects `roe` and `balance_sheet` both sorted ascending.
/// avg_equity = (equity at year_end + equity at prior year_end) / 2.
pub fn compute_roe(roe: &mut Vec<ReturnOnEquityHistory>, balance_sheet: &[BalanceSheetHistory]) {
    for r in roe.iter_mut() {
        // Find equity entries closest to this and prior fiscal year end
        let eq_now = nearest_equity(balance_sheet, r.period_of_report);
        let prior = r
            .period_of_report
            .with_year(r.period_of_report.year() - 1)
            .unwrap_or(r.period_of_report);
        let eq_prior = nearest_equity(balance_sheet, prior);

        r.avg_equity = match (eq_now, eq_prior) {
            (Some(a), Some(b)) => Some((a + b) / 2),
            (Some(a), None) => Some(a),
            _ => None,
        };

        r.roe_pct = match (r.net_income_ttm, r.avg_equity) {
            (Some(ni), Some(eq)) if eq != 0 => Some(round3(ni as f64 / eq as f64 * 100.0)),
            _ => None,
        };
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn find_yoy_index(dates: &[NaiveDate], current: NaiveDate) -> Option<usize> {
    let target = current - chrono::Duration::days(365);
    dates
        .iter()
        .enumerate()
        .filter(|(_, d)| (**d - target).num_days().abs() <= 30)
        .min_by_key(|(_, d)| (**d - target).num_days().abs())
        .map(|(i, _)| i)
}

fn nearest_equity(bs: &[BalanceSheetHistory], target: NaiveDate) -> Option<i64> {
    bs.iter()
        .filter(|b| (b.period_of_report - target).num_days().abs() <= 120)
        .min_by_key(|b| (b.period_of_report - target).num_days().abs())
        .and_then(|b| b.stockholders_equity)
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn pct_change(current: Option<f64>, prior: Option<f64>) -> Option<f64> {
    match (current, prior) {
        (Some(c), Some(p)) if p != 0.0 => Some(round3((c - p) / p.abs() * 100.0)),
        _ => None,
    }
}

fn pct_change_i64(current: Option<i64>, prior: Option<i64>) -> Option<f64> {
    match (current, prior) {
        (Some(c), Some(p)) if p != 0 => Some(round3((c - p) as f64 / p.abs() as f64 * 100.0)),
        _ => None,
    }
}

fn yoy_growth(current: Option<f64>, year_ago: Option<f64>) -> Option<f64> {
    pct_change(current, year_ago)
}

fn yoy_growth_i64(current: Option<i64>, year_ago: Option<i64>) -> Option<f64> {
    pct_change_i64(current, year_ago)
}

fn pct_of(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(n), Some(d)) if d != 0.0 => Some(round3(n / d * 100.0)),
        _ => None,
    }
}
