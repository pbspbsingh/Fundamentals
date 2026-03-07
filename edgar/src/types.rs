use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsHistory {
    pub period_of_report: NaiveDate,
    pub fiscal_quarter: u8,
    pub fiscal_year: u16,
    pub eps_diluted: Option<f64>,
    pub eps_basic: Option<f64>,
    pub net_income: Option<i64>,
    pub diluted_shares_outstanding: Option<i64>,
    /// Computed: YoY % change in EPS diluted
    pub eps_yoy_growth: Option<f64>,
    /// Computed: QoQ % change in EPS diluted
    pub eps_qoq_growth: Option<f64>,
    /// Computed: change in YoY growth rate vs prior quarter
    pub eps_acceleration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueHistory {
    pub period_of_report: NaiveDate,
    pub fiscal_quarter: u8,
    pub fiscal_year: u16,
    pub revenue: Option<i64>,
    /// Computed
    pub revenue_yoy_growth: Option<f64>,
    /// Computed
    pub revenue_qoq_growth: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginHistory {
    pub period_of_report: NaiveDate,
    pub gross_profit: Option<i64>,
    pub cost_of_revenue: Option<i64>,
    pub operating_income: Option<i64>,
    pub net_income: Option<i64>,
    pub revenue: Option<i64>,
    /// Computed
    pub gross_margin_pct: Option<f64>,
    /// Computed
    pub operating_margin_pct: Option<f64>,
    /// Computed
    pub net_margin_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowHistory {
    pub period_of_report: NaiveDate,
    pub fiscal_year: u16,
    pub operating_cash_flow: Option<i64>,
    /// Capex (payments to acquire PP&E)
    pub capex: Option<i64>,
    /// Computed: operating_cash_flow - capex
    pub free_cash_flow: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharesHistory {
    pub period_of_report: NaiveDate,
    pub shares_outstanding: Option<i64>,
    pub shares_diluted: Option<i64>,
    /// Computed: positive = dilution, negative = buyback
    pub qoq_change_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetHistory {
    pub period_of_report: NaiveDate,
    pub cash: Option<i64>,
    pub long_term_debt: Option<i64>,
    pub short_term_debt: Option<i64>,
    /// Computed: long_term_debt + short_term_debt
    pub total_debt: Option<i64>,
    pub stockholders_equity: Option<i64>,
    /// Computed: total_debt / stockholders_equity
    pub debt_to_equity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnOnEquityHistory {
    pub period_of_report: NaiveDate,
    pub fiscal_year: u16,
    pub net_income_ttm: Option<i64>,
    pub avg_equity: Option<i64>,
    /// Computed: net_income_ttm / avg_equity * 100
    pub roe_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsiderTransaction {
    pub transaction_date: NaiveDate,
    pub insider_name: String,
    /// "Officer", "Director", "10% Owner", etc.
    pub insider_role: String,
    /// true only for open-market buys (P) or sells (S)
    pub is_open_market: bool,
    pub shares: i64,
    pub price_per_share: Option<f64>,
    /// 'A' = acquisition, 'D' = disposition
    pub acquisition_or_disposition: char,
    /// Computed: shares * price_per_share
    pub total_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgarFundamentals {
    pub ticker: String,
    pub cik: String,
    pub company_name: String,
    pub fetched_at: DateTime<Utc>,
    /// Last 12 quarters, ascending
    pub earnings: Vec<EarningsHistory>,
    /// Last 12 quarters, ascending
    pub revenue: Vec<RevenueHistory>,
    /// Last 12 quarters, ascending
    pub margins: Vec<MarginHistory>,
    /// Last 4 annual periods, ascending
    pub cash_flows: Vec<CashFlowHistory>,
    /// Last 12 quarters, ascending
    pub shares: Vec<SharesHistory>,
    /// Last 8 quarters, ascending
    pub balance_sheet: Vec<BalanceSheetHistory>,
    /// Last 4 annual periods, ascending
    pub roe: Vec<ReturnOnEquityHistory>,
    /// Last 12 months, ascending by transaction date
    pub insider_transactions: Vec<InsiderTransaction>,
}
