use chrono::NaiveDate;
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

    pub cash_and_equivalents: Option<f64>,
    pub short_term_investments: Option<f64>,
    pub total_cash_and_short_term_investments: Option<f64>,
    pub accounts_receivable: Option<f64>,
    pub total_receivables: Option<f64>,
    pub inventory: Option<f64>,
    pub other_current_assets: Option<f64>,
    pub total_current_assets: Option<f64>,
    pub gross_ppe: Option<f64>,
    pub accumulated_depreciation: Option<f64>,
    pub net_ppe: Option<f64>,
    pub goodwill: Option<f64>,
    pub intangible_assets: Option<f64>,
    pub other_long_term_assets: Option<f64>,
    pub total_assets: Option<f64>,
    pub accounts_payable: Option<f64>,
    pub short_term_debt: Option<f64>,
    pub other_current_liabilities: Option<f64>,
    pub total_current_liabilities: Option<f64>,
    pub long_term_debt: Option<f64>,
    pub other_long_term_liabilities: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub common_stock: Option<f64>,
    pub retained_earnings: Option<f64>,
    pub total_stockholders_equity: Option<f64>,
    pub total_equity: Option<f64>,
    pub total_liabilities_and_equity: Option<f64>,
}

// ── Cash Flow ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowEntry {
    pub period: Period,

    pub net_income: Option<f64>,
    pub depreciation_and_amortization: Option<f64>,
    pub stock_based_compensation: Option<f64>,
    pub change_in_working_capital: Option<f64>,
    pub other_operating_activities: Option<f64>,
    pub operating_cash_flow: Option<f64>,
    pub operating_cash_flow_yoy: Option<f64>,
    pub capex: Option<f64>,
    pub acquisitions: Option<f64>,
    pub other_investing_activities: Option<f64>,
    pub investing_cash_flow: Option<f64>,
    pub dividends_paid: Option<f64>,
    pub share_issuance_repurchase: Option<f64>,
    pub debt_issuance_repayment: Option<f64>,
    pub other_financing_activities: Option<f64>,
    pub financing_cash_flow: Option<f64>,
    pub net_change_in_cash: Option<f64>,
    pub free_cash_flow: Option<f64>,
    pub free_cash_flow_yoy: Option<f64>,
}

// ── Top-level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingViewFinancials {
    pub ticker: String,
    pub currency: String,
    pub periodicity: Periodicity,
    pub scraped_at: chrono::DateTime<chrono::Utc>,

    /// Chronological order, oldest first
    pub income_statement: Vec<IncomeStatementEntry>,
    pub balance_sheet: Vec<BalanceSheetEntry>,
    pub cash_flow: Vec<CashFlowEntry>,

    pub ttm_income: Option<IncomeStatementEntry>,
    pub ttm_cash_flow: Option<CashFlowEntry>,
}
