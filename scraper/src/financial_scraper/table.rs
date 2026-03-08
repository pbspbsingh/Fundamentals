use anyhow::Context;
use chrono::{Local, NaiveDate};
use model::financials::{
    BalanceSheetEntry, CashFlowEntry, IncomeStatementEntry, Period, Periodicity, StatisticsEntry,
};

use super::utils::{parse_month_year, parse_pct, parse_value, round3};

pub(super) struct TableData<'a>(pub(super) &'a serde_json::Value);

impl<'a> TableData<'a> {
    pub(super) fn val(&self, i: usize, name: &str) -> Option<f64> {
        parse_value(self.0[name][i]["value"].as_str().unwrap_or(""))
    }
    pub(super) fn chg(&self, i: usize, name: &str) -> Option<f64> {
        parse_pct(self.0[name][i]["change"].as_str().unwrap_or(""))
    }
    /// For margin/return fields displayed as "81.67" (already ×100); divides by 100.
    pub(super) fn pct_val(&self, i: usize, name: &str) -> Option<f64> {
        self.val(i, name).map(|x| round3(x / 100.0))
    }
    pub(super) fn locked(&self, i: usize, name: &str) -> bool {
        self.0[name][i]["locked"].as_bool().unwrap_or(false)
    }
}

pub(super) fn collect_entries<T: HasPeriodEnd>(
    columns: &[serde_json::Value],
    td: &TableData<'_>,
    is_quarterly: bool,
    lock_field: &str,
    build: impl Fn(&TableData<'_>, usize, Period) -> T,
) -> Vec<T> {
    let periodicity = if is_quarterly {
        Periodicity::Quarterly
    } else {
        Periodicity::Annual
    };
    let mut entries: Vec<T> = columns
        .iter()
        .enumerate()
        .filter_map(|(i, col)| {
            let date = parse_month_year(col["date"].as_str()?)?;
            if td.locked(i, lock_field) {
                return None;
            }
            let label = col["label"].as_str().unwrap_or("").to_string();
            Some(build(
                td,
                i,
                Period {
                    label,
                    period_end: Some(date),
                    periodicity,
                },
            ))
        })
        .collect();
    entries.sort_by_key(|e| e.period_end());
    entries
}

pub(super) fn find_ttm_col(
    columns: &[serde_json::Value],
    td: &TableData<'_>,
    lock_field: &str,
) -> anyhow::Result<(usize, NaiveDate, String)> {
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
    if td.locked(i, lock_field) {
        anyhow::bail!("TTM column (index {i} of {}) is paywalled", columns.len());
    }
    let label = columns[i]["label"].as_str().unwrap_or("TTM").to_string();
    let period_end = columns
        .iter()
        .filter_map(|col| parse_month_year(col["date"].as_str()?))
        .max()
        .unwrap_or_else(|| Local::now().date_naive());
    Ok((i, period_end, label))
}

pub(super) trait HasPeriodEnd {
    fn period_end(&self) -> NaiveDate;
}

impl HasPeriodEnd for IncomeStatementEntry {
    fn period_end(&self) -> NaiveDate {
        self.period.period_end.unwrap_or_default()
    }
}
impl HasPeriodEnd for BalanceSheetEntry {
    fn period_end(&self) -> NaiveDate {
        self.period.period_end.unwrap_or_default()
    }
}
impl HasPeriodEnd for CashFlowEntry {
    fn period_end(&self) -> NaiveDate {
        self.period.period_end.unwrap_or_default()
    }
}
impl HasPeriodEnd for StatisticsEntry {
    fn period_end(&self) -> NaiveDate {
        self.period.period_end.unwrap_or_default()
    }
}
