use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// SEC form type, e.g. "8-K"
    pub form_type: String,
    /// Date the filing was submitted
    pub filed_at: NaiveDate,
    /// Primary document description from EDGAR (e.g. "CURRENT REPORT")
    pub description: String,
    /// True if this 8-K contains earnings results (has Exhibit 99.1 / 99.2)
    pub is_earnings_release: bool,
    /// Markdown of the 8-K cover page
    pub cover_page: Option<String>,
    /// Exhibit 99.1 — press release content as Markdown
    pub press_release: Option<String>,
    /// Exhibit 99.2 — CFO commentary content as Markdown
    pub cfo_commentary: Option<String>,
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
