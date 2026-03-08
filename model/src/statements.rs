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
