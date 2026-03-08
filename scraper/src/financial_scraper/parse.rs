use model::financials::{
    BalanceSheetEntry, CashFlowEntry, IncomeStatementEntry, Period, Periodicity, StatisticsEntry,
};

use super::FinancialScraper;
use super::table::{TableData, collect_entries, find_ttm_col};

impl FinancialScraper {
    pub(super) async fn parse_income_statement(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<IncomeStatementEntry>> {
        let (columns, rows) = self.fetch_table().await?;
        let td = TableData(&rows);
        Ok(collect_entries(&columns, &td, is_quarterly, "Total revenue", |td, i, period| {
            IncomeStatementEntry {
                period,
                total_revenue: td.val(i, "Total revenue"),
                total_revenue_yoy: td.chg(i, "Total revenue"),
                cost_of_goods_sold: td.val(i, "Cost of goods sold"),
                gross_profit: td.val(i, "Gross profit"),
                operating_expenses_excl_cogs: td.val(i, "Operating expenses (excl. COGS)"),
                operating_income: td.val(i, "Operating income"),
                operating_income_yoy: td.chg(i, "Operating income"),
                non_operating_income: td.val(i, "Non-operating income (total)"),
                pretax_income: td.val(i, "Pretax income"),
                pretax_income_yoy: td.chg(i, "Pretax income"),
                equity_in_earnings: td.val(i, "Equity in earnings"),
                taxes: td.val(i, "Taxes"),
                minority_interest: td.val(i, "Non-controlling/minority interest"),
                after_tax_other_income: td.val(i, "After tax other income/expense"),
                net_income_before_discontinued: td.val(i, "Net income before discontinued operations"),
                discontinued_operations: td.val(i, "Discontinued operations"),
                net_income: td.val(i, "Net income"),
                net_income_yoy: td.chg(i, "Net income"),
                dilution_adjustment: td.val(i, "Dilution adjustment"),
                preferred_dividends: td.val(i, "Preferred dividends"),
                net_income_available_to_common: td.val(i, "Diluted net income available to common stockholders"),
                eps_basic: td.val(i, "Basic earnings per share (basic EPS)"),
                eps_basic_yoy: td.chg(i, "Basic earnings per share (basic EPS)"),
                eps_diluted: td.val(i, "Diluted earnings per share (diluted EPS)"),
                eps_diluted_yoy: td.chg(i, "Diluted earnings per share (diluted EPS)"),
                shares_basic: td.val(i, "Average basic shares outstanding"),
                shares_diluted: td.val(i, "Diluted shares outstanding"),
                ebitda: td.val(i, "EBITDA"),
                ebit: td.val(i, "EBIT"),
                ebit_yoy: td.chg(i, "EBIT"),
                total_operating_expenses: td.val(i, "Total operating expenses"),
            }
        }))
    }

    pub(super) async fn parse_ttm_income(&self) -> anyhow::Result<IncomeStatementEntry> {
        let (columns, rows) = self.fetch_table().await?;
        let td = TableData(&rows);
        let (i, period_end) = find_ttm_col(&columns, &td, "Total revenue")?;
        Ok(IncomeStatementEntry {
            period: Period { period_end, periodicity: Periodicity::Annual },
            total_revenue: td.val(i, "Total revenue"),
            total_revenue_yoy: td.chg(i, "Total revenue"),
            cost_of_goods_sold: td.val(i, "Cost of goods sold"),
            gross_profit: td.val(i, "Gross profit"),
            operating_expenses_excl_cogs: td.val(i, "Operating expenses (excl. COGS)"),
            operating_income: td.val(i, "Operating income"),
            operating_income_yoy: td.chg(i, "Operating income"),
            non_operating_income: td.val(i, "Non-operating income (total)"),
            pretax_income: td.val(i, "Pretax income"),
            pretax_income_yoy: td.chg(i, "Pretax income"),
            equity_in_earnings: td.val(i, "Equity in earnings"),
            taxes: td.val(i, "Taxes"),
            minority_interest: td.val(i, "Non-controlling/minority interest"),
            after_tax_other_income: td.val(i, "After tax other income/expense"),
            net_income_before_discontinued: td.val(i, "Net income before discontinued operations"),
            discontinued_operations: td.val(i, "Discontinued operations"),
            net_income: td.val(i, "Net income"),
            net_income_yoy: td.chg(i, "Net income"),
            dilution_adjustment: td.val(i, "Dilution adjustment"),
            preferred_dividends: td.val(i, "Preferred dividends"),
            net_income_available_to_common: td.val(i, "Diluted net income available to common stockholders"),
            eps_basic: td.val(i, "Basic earnings per share (basic EPS)"),
            eps_basic_yoy: td.chg(i, "Basic earnings per share (basic EPS)"),
            eps_diluted: td.val(i, "Diluted earnings per share (diluted EPS)"),
            eps_diluted_yoy: td.chg(i, "Diluted earnings per share (diluted EPS)"),
            shares_basic: td.val(i, "Average basic shares outstanding"),
            shares_diluted: td.val(i, "Diluted shares outstanding"),
            ebitda: td.val(i, "EBITDA"),
            ebit: td.val(i, "EBIT"),
            ebit_yoy: td.chg(i, "EBIT"),
            total_operating_expenses: td.val(i, "Total operating expenses"),
        })
    }

    pub(super) async fn parse_balance_sheet(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<BalanceSheetEntry>> {
        let (columns, rows) = self.fetch_table().await?;
        let td = TableData(&rows);
        Ok(collect_entries(&columns, &td, is_quarterly, "Total assets", |td, i, period| {
            BalanceSheetEntry {
                period,
                total_assets: td.val(i, "Total assets"),
                total_assets_yoy: td.chg(i, "Total assets"),
                total_liabilities: td.val(i, "Total liabilities"),
                total_liabilities_yoy: td.chg(i, "Total liabilities"),
                total_equity: td.val(i, "Total equity"),
                total_equity_yoy: td.chg(i, "Total equity"),
                total_liabilities_and_equity: td.val(i, "Total liabilities & shareholders' equities"),
                total_debt: td.val(i, "Total debt"),
                net_debt: td.val(i, "Net debt"),
            }
        }))
    }

    pub(super) async fn parse_cash_flow(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<CashFlowEntry>> {
        let (columns, rows) = self.fetch_table().await?;
        let td = TableData(&rows);
        Ok(collect_entries(&columns, &td, is_quarterly, "Cash from operating activities", |td, i, period| {
            CashFlowEntry {
                period,
                operating_cash_flow: td.val(i, "Cash from operating activities"),
                operating_cash_flow_yoy: td.chg(i, "Cash from operating activities"),
                investing_cash_flow: td.val(i, "Cash from investing activities"),
                investing_cash_flow_yoy: td.chg(i, "Cash from investing activities"),
                financing_cash_flow: td.val(i, "Cash from financing activities"),
                financing_cash_flow_yoy: td.chg(i, "Cash from financing activities"),
                free_cash_flow: td.val(i, "Free cash flow"),
                free_cash_flow_yoy: td.chg(i, "Free cash flow"),
            }
        }))
    }

    pub(super) async fn parse_ttm_cash_flow(&self) -> anyhow::Result<CashFlowEntry> {
        let (columns, rows) = self.fetch_table().await?;
        let td = TableData(&rows);
        let (i, period_end) = find_ttm_col(&columns, &td, "Cash from operating activities")?;
        Ok(CashFlowEntry {
            period: Period { period_end, periodicity: Periodicity::Annual },
            operating_cash_flow: td.val(i, "Cash from operating activities"),
            operating_cash_flow_yoy: td.chg(i, "Cash from operating activities"),
            investing_cash_flow: td.val(i, "Cash from investing activities"),
            investing_cash_flow_yoy: td.chg(i, "Cash from investing activities"),
            financing_cash_flow: td.val(i, "Cash from financing activities"),
            financing_cash_flow_yoy: td.chg(i, "Cash from financing activities"),
            free_cash_flow: td.val(i, "Free cash flow"),
            free_cash_flow_yoy: td.chg(i, "Free cash flow"),
        })
    }

    pub(super) async fn parse_statistics(
        &self,
        is_quarterly: bool,
    ) -> anyhow::Result<Vec<StatisticsEntry>> {
        let (columns, rows) = self.fetch_table().await?;
        let td = TableData(&rows);
        Ok(collect_entries(&columns, &td, is_quarterly, "Total common shares outstanding", |td, i, period| {
            StatisticsEntry {
                period,
                shares_outstanding: td.val(i, "Total common shares outstanding"),
                free_float: td.val(i, "Free float"),
                employee_count: td.val(i, "Number of employees"),
                shareholder_count: td.val(i, "Number of shareholders"),
                enterprise_value: td.val(i, "Enterprise value"),
                pe_ratio: td.val(i, "Price to earnings ratio"),
                ps_ratio: td.val(i, "Price to sales ratio"),
                pb_ratio: td.val(i, "Price to book ratio"),
                pcf_ratio: td.val(i, "Price to cash flow ratio"),
                ev_to_ebitda: td.val(i, "Enterprise value to EBITDA ratio"),
                gross_margin: td.pct_val(i, "Gross margin %"),
                operating_margin: td.pct_val(i, "Operating margin %"),
                ebitda_margin: td.pct_val(i, "EBITDA margin %"),
                net_margin: td.pct_val(i, "Net margin %"),
                return_on_assets: td.pct_val(i, "Return on assets %"),
                return_on_equity: td.pct_val(i, "Return on equity %"),
                return_on_invested_capital: td.pct_val(i, "Return on invested capital %"),
                current_ratio: td.val(i, "Current ratio"),
                quick_ratio: td.val(i, "Quick ratio"),
                debt_to_equity: td.val(i, "Debt to equity ratio"),
                debt_to_assets: td.val(i, "Debt to assets ratio"),
                lt_debt_to_equity: td.val(i, "Long term debt to total equity ratio"),
                lt_debt_to_assets: td.val(i, "Long term debt to total assets ratio"),
                asset_turnover: td.val(i, "Asset turnover"),
                inventory_turnover: td.val(i, "Inventory turnover"),
                revenue_per_share: td.val(i, "Revenue per share"),
                ocf_per_share: td.val(i, "Operating cash flow per share"),
                fcf_per_share: td.val(i, "Free cash flow per share"),
                ebit_per_share: td.val(i, "EBIT per share"),
                ebitda_per_share: td.val(i, "EBITDA per share"),
                book_value_per_share: td.val(i, "Book value per share"),
                tangible_book_value_per_share: td.val(i, "Tangible book value per share"),
                net_current_asset_value_per_share: td.val(i, "Net current asset value per share"),
                working_capital_per_share: td.val(i, "Working capital per share"),
                cash_per_share: td.val(i, "Cash per share"),
                total_debt_per_share: td.val(i, "Total debt per share"),
                capex_per_share: td.val(i, "CapEx per share"),
            }
        }))
    }
}
