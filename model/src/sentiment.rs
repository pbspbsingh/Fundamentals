use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockSentiment {
    pub sector: String,
    pub industry: String,
    pub about: String,
    pub analyst_news: Vec<NewsHeadline>,
    pub forecast: Option<Forecast>,
    // Metadata
    pub scraped_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewsHeadline {
    pub headline: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Forecast {
    // Price target
    pub price_current: Option<f64>,
    pub price_target_average: Option<f64>,
    pub price_target_average_upside_pct: Option<f64>,
    pub price_target_max: Option<f64>,
    pub price_target_max_upside_pct: Option<f64>,
    pub price_target_min: Option<f64>,
    pub price_target_min_downside_pct: Option<f64>,
    pub price_target_analyst_count: Option<u32>,

    // Analyst rating
    pub rating_strong_buy: Option<u32>,
    pub rating_buy: Option<u32>,
    pub rating_hold: Option<u32>,
    pub rating_sell: Option<u32>,
    pub rating_strong_sell: Option<u32>,
    pub rating_total_analysts: Option<u32>,
    pub rating_consensus: Option<String>,
}
