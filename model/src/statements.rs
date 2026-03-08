use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

// ── Shared ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Periodicity {
    Annual,
    Quarterly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    pub period_end: NaiveDate,
    pub periodicity: Periodicity,
}

// ── Income Statement ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatementEntry {
    pub period: Period,

    pub total_revenue: Option<f64>,
    pub total_revenue_yoy: Option<f64>,
    pub cost_of_goods_sold: Option<f64>,
    pub gross_profit: Option<f64>,
    pub operating_expenses_excl_cogs: Option<f64>,
    pub operating_income: Option<f64>,
    pub operating_income_yoy: Option<f64>,
    pub non_operating_income: Option<f64>,
    pub pretax_income: Option<f64>,
    pub pretax_income_yoy: Option<f64>,
    pub equity_in_earnings: Option<f64>,
    pub taxes: Option<f64>,
    pub minority_interest: Option<f64>,
    pub after_tax_other_income: Option<f64>,
    pub net_income_before_discontinued: Option<f64>,
    pub discontinued_operations: Option<f64>,
    pub net_income: Option<f64>,
    pub net_income_yoy: Option<f64>,
    pub dilution_adjustment: Option<f64>,
    pub preferred_dividends: Option<f64>,
    pub net_income_available_to_common: Option<f64>,
    pub eps_basic: Option<f64>,
    pub eps_basic_yoy: Option<f64>,
    pub eps_diluted: Option<f64>,
    pub eps_diluted_yoy: Option<f64>,
    pub shares_basic: Option<f64>,
    pub shares_diluted: Option<f64>,
    pub ebitda: Option<f64>,
    pub ebit: Option<f64>,
    pub ebit_yoy: Option<f64>,
    pub total_operating_expenses: Option<f64>,
}

// ── Balance Sheet ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetEntry {
    pub period: Period,

    pub total_assets: Option<f64>,
    pub total_assets_yoy: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_liabilities_yoy: Option<f64>,
    pub total_equity: Option<f64>,
    pub total_equity_yoy: Option<f64>,
    pub total_liabilities_and_equity: Option<f64>,
    pub total_debt: Option<f64>,
    pub net_debt: Option<f64>,
}

// ── Cash Flow ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowEntry {
    pub period: Period,

    pub operating_cash_flow: Option<f64>,
    pub operating_cash_flow_yoy: Option<f64>,
    pub investing_cash_flow: Option<f64>,
    pub investing_cash_flow_yoy: Option<f64>,
    pub financing_cash_flow: Option<f64>,
    pub financing_cash_flow_yoy: Option<f64>,
    pub free_cash_flow: Option<f64>,
    pub free_cash_flow_yoy: Option<f64>,
}

// ── Top-level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingViewFinancials {
    pub ticker: String,
    pub currency: String,
    pub scraped_at: DateTime<Local>,

    pub quarterly_income: Vec<IncomeStatementEntry>,
    pub annual_income: Vec<IncomeStatementEntry>,

    pub quarterly_balance_sheet: Vec<BalanceSheetEntry>,
    pub annual_balance_sheet: Vec<BalanceSheetEntry>,

    pub quarterly_cash_flow: Vec<CashFlowEntry>,
    pub annual_cash_flow: Vec<CashFlowEntry>,

    pub ttm_income: Option<IncomeStatementEntry>,
    pub ttm_cash_flow: Option<CashFlowEntry>,
}
