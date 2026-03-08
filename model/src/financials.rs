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
    pub label: String,
    pub period_end: NaiveDate,
    pub periodicity: Periodicity,
}

// ── Income Statement ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatementEntry {
    pub period: Period,

    pub total_revenue: Option<f64>,
    #[serde(with = "pct_serde")]
    pub total_revenue_yoy: Option<f64>,
    pub cost_of_goods_sold: Option<f64>,
    pub gross_profit: Option<f64>,
    pub operating_expenses_excl_cogs: Option<f64>,
    pub operating_income: Option<f64>,
    #[serde(with = "pct_serde")]
    pub operating_income_yoy: Option<f64>,
    pub non_operating_income: Option<f64>,
    pub pretax_income: Option<f64>,
    #[serde(with = "pct_serde")]
    pub pretax_income_yoy: Option<f64>,
    pub equity_in_earnings: Option<f64>,
    pub taxes: Option<f64>,
    pub minority_interest: Option<f64>,
    pub after_tax_other_income: Option<f64>,
    pub net_income_before_discontinued: Option<f64>,
    pub discontinued_operations: Option<f64>,
    pub net_income: Option<f64>,
    #[serde(with = "pct_serde")]
    pub net_income_yoy: Option<f64>,
    pub dilution_adjustment: Option<f64>,
    pub preferred_dividends: Option<f64>,
    pub net_income_available_to_common: Option<f64>,
    pub eps_basic: Option<f64>,
    #[serde(with = "pct_serde")]
    pub eps_basic_yoy: Option<f64>,
    pub eps_diluted: Option<f64>,
    #[serde(with = "pct_serde")]
    pub eps_diluted_yoy: Option<f64>,
    pub shares_basic: Option<f64>,
    pub shares_diluted: Option<f64>,
    pub ebitda: Option<f64>,
    pub ebit: Option<f64>,
    #[serde(with = "pct_serde")]
    pub ebit_yoy: Option<f64>,
    pub total_operating_expenses: Option<f64>,
}

// ── Balance Sheet ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetEntry {
    pub period: Period,

    pub total_assets: Option<f64>,
    #[serde(with = "pct_serde")]
    pub total_assets_yoy: Option<f64>,
    pub total_liabilities: Option<f64>,
    #[serde(with = "pct_serde")]
    pub total_liabilities_yoy: Option<f64>,
    pub total_equity: Option<f64>,
    #[serde(with = "pct_serde")]
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
    #[serde(with = "pct_serde")]
    pub operating_cash_flow_yoy: Option<f64>,
    pub investing_cash_flow: Option<f64>,
    #[serde(with = "pct_serde")]
    pub investing_cash_flow_yoy: Option<f64>,
    pub financing_cash_flow: Option<f64>,
    #[serde(with = "pct_serde")]
    pub financing_cash_flow_yoy: Option<f64>,
    pub free_cash_flow: Option<f64>,
    #[serde(with = "pct_serde")]
    pub free_cash_flow_yoy: Option<f64>,
}

// ── Statistics ────────────────────────────────────────────────────────────────
// Margin/return fields are stored as fractions (0.8167 for 81.67%).
// Ratio/valuation fields are stored as raw multiples (e.g. P/E = 181.61).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsEntry {
    pub period: Period,

    // Shares & company info
    pub shares_outstanding: Option<f64>,
    pub free_float: Option<f64>,
    pub employee_count: Option<f64>,
    pub shareholder_count: Option<f64>,

    // Valuation
    pub enterprise_value: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub ps_ratio: Option<f64>,
    pub pb_ratio: Option<f64>,
    pub pcf_ratio: Option<f64>,
    pub ev_to_ebitda: Option<f64>,

    // Profitability (0–1 fractions)
    pub gross_margin: Option<f64>,
    pub operating_margin: Option<f64>,
    pub ebitda_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub return_on_invested_capital: Option<f64>,

    // Liquidity & Leverage
    pub current_ratio: Option<f64>,
    pub quick_ratio: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub debt_to_assets: Option<f64>,
    pub lt_debt_to_equity: Option<f64>,
    pub lt_debt_to_assets: Option<f64>,
    pub asset_turnover: Option<f64>,
    pub inventory_turnover: Option<f64>,

    // Per Share
    pub revenue_per_share: Option<f64>,
    pub ocf_per_share: Option<f64>,
    pub fcf_per_share: Option<f64>,
    pub ebit_per_share: Option<f64>,
    pub ebitda_per_share: Option<f64>,
    pub book_value_per_share: Option<f64>,
    pub tangible_book_value_per_share: Option<f64>,
    pub net_current_asset_value_per_share: Option<f64>,
    pub working_capital_per_share: Option<f64>,
    pub cash_per_share: Option<f64>,
    pub total_debt_per_share: Option<f64>,
    pub capex_per_share: Option<f64>,
}

// ── Earnings ──────────────────────────────────────────────────────────────────

/// One column from the Earnings tab (EPS + Revenue combined).
/// `period_end` is the last day of the fiscal quarter/year derived from the
/// TradingView label (e.g. "Q1 '21" → 2021-03-31, "FY '21" → 2021-12-31).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsEntry {
    pub period: Period,

    pub eps_reported: Option<f64>,
    pub eps_estimate: Option<f64>,
    #[serde(with = "pct_serde")]
    pub eps_surprise: Option<f64>,

    pub revenue_reported: Option<f64>,
    pub revenue_estimate: Option<f64>,
    #[serde(with = "pct_serde")]
    pub revenue_surprise: Option<f64>,
}

// ── Top-level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingViewFinancials {
    pub ticker: String,
    pub currency: String,

    pub quarterly_income: Vec<IncomeStatementEntry>,
    pub annual_income: Vec<IncomeStatementEntry>,

    pub quarterly_balance_sheet: Vec<BalanceSheetEntry>,
    pub annual_balance_sheet: Vec<BalanceSheetEntry>,

    pub quarterly_cash_flow: Vec<CashFlowEntry>,
    pub annual_cash_flow: Vec<CashFlowEntry>,

    pub ttm_income: Option<IncomeStatementEntry>,
    pub ttm_cash_flow: Option<CashFlowEntry>,

    pub quarterly_statistics: Vec<StatisticsEntry>,
    pub annual_statistics: Vec<StatisticsEntry>,

    pub quarterly_earnings: Vec<EarningsEntry>,
    pub annual_earnings: Vec<EarningsEntry>,
}

// ── Percentage serde ───────────────────────────────────────────────────────────
// Serializes Option<f64> fractional growth (e.g. 0.208) as "20.8%".
// Deserializes from either a percentage string or a raw float (for back-compat).

mod pct_serde {
    use serde::{Deserializer, Serializer, de};

    fn fmt_pct(v: f64) -> String {
        let s = format!("{:.3}", v * 100.0);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{s}%")
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
        struct V;

        impl<'de> de::Visitor<'de> for V {
            type Value = Option<f64>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("null, float, or percentage string like \"20.8%\"")
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(Some(v as f64))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(Some(v as f64))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(Some(v))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                v.trim_end_matches('%')
                    .parse::<f64>()
                    .map(|n| Some(n / 100.0))
                    .map_err(E::custom)
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                d.deserialize_any(self)
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
        }
        d.deserialize_option(V)
    }

    pub fn serialize<S: Serializer>(val: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
        match val {
            Some(v) => s.serialize_str(&fmt_pct(*v)),
            None => s.serialize_none(),
        }
    }
}
