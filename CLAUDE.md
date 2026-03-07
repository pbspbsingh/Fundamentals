# Fundamentals

A Rust workspace that scrapes fundamental data (description, IPO date, last earnings date) for stock tickers from TradingView using a Chrome browser automation, and fetches SEC filings via the EDGAR API.

## Workspace Structure

- `runner/` — binary entry point; hardcodes tickers and orchestrates fetching
- `scraper/` — TradingView scraper using `chrome_driver` (headless Chrome automation)
- `edgar/` — SEC EDGAR REST API client (`EdgarClient`)
- `model/` — shared data types (`Ticker`, `Fundamentals`)
- `config/` — loads `config.toml` via `OnceLock`; call `config::config()` anywhere

## Build & Run

```bash
cargo build
cargo run                   # runs runner, reads config.toml from CWD
RUST_LOG=debug cargo run    # verbose logging
```

Config is read from `config.toml` in the working directory (must be project root).

## Key Files

| File | Purpose |
|------|---------|
| `config.toml` | Chrome path, user data dir, log level |
| `runner/src/main.rs` | Entry point, sets up tracing, calls scraper |
| `scraper/src/lib.rs` | `start_fetching(ticker)` — browser scrape |
| `edgar/src/client.rs` | `EdgarClient::new()` — SEC HTTP client |
| `model/src/lib.rs` | `Ticker`, `Fundamentals` structs |
| `config/src/lib.rs` | `config()` singleton |

## Plan

Update edgar module so that it fetches and parses fundamental data from the SEC EDGAR API for a given stock ticker, populating a comprehensive struct suitable for CANSLIM, SEPA, and Episodic Pivot analysis.

## Requirements

### Struct Design

Create a master struct `EdgarFundamentals` with the following nested structs:

**`EarningsHistory`** — vec of quarterly snapshots containing:
- period_of_report (NaiveDate)
- fiscal_quarter (u8), fiscal_year (u16)
- eps_diluted (Option<f64>)
- eps_basic (Option<f64>)
- net_income (Option<i64>)
- diluted_shares_outstanding (Option<i64>)
- eps_yoy_growth (Option<f64>)        // computed
- eps_qoq_growth (Option<f64>)        // computed
- eps_acceleration (Option<f64>)       // change in growth rate vs prior quarter

**`RevenueHistory`** — vec of quarterly snapshots:
- period_of_report (NaiveDate)
- fiscal_quarter (u8), fiscal_year (u16)
- revenue (Option<i64>)
- revenue_yoy_growth (Option<f64>)    // computed
- revenue_qoq_growth (Option<f64>)    // computed

**`MarginHistory`** — vec of quarterly snapshots:
- period_of_report (NaiveDate)
- gross_profit (Option<i64>)
- cost_of_revenue (Option<i64>)
- operating_income (Option<i64>)
- net_income (Option<i64>)
- gross_margin_pct (Option<f64>)      // computed
- operating_margin_pct (Option<f64>)  // computed
- net_margin_pct (Option<f64>)        // computed

**`CashFlowHistory`** — vec of annual snapshots:
- period_of_report (NaiveDate)
- operating_cash_flow (Option<i64>)
- capex (Option<i64>)
- free_cash_flow (Option<i64>)        // computed: operating_cf - capex

**`SharesHistory`** — vec of quarterly snapshots:
- period_of_report (NaiveDate)
- shares_outstanding (Option<i64>)
- shares_diluted (Option<i64>)
- qoq_change_pct (Option<f64>)        // computed: positive = dilution, negative = buyback

**`BalanceSheetHistory`** — vec of quarterly snapshots:
- period_of_report (NaiveDate)
- cash (Option<i64>)
- long_term_debt (Option<i64>)
- short_term_debt (Option<i64>)
- total_debt (Option<i64>)            // computed
- stockholders_equity (Option<i64>)
- debt_to_equity (Option<f64>)        // computed

**`ReturnOnEquityHistory`** — vec of annual snapshots:
- period_of_report (NaiveDate)
- net_income_ttm (Option<i64>)
- avg_equity (Option<i64>)
- roe_pct (Option<f64>)               // computed: net_income_ttm / avg_equity * 100

**`InsiderTransaction`** — vec of Form 4 entries:
- transaction_date (NaiveDate)
- insider_name (String)
- insider_role (String)               // Officer, Director, 10% Owner
- is_open_market (bool)               // exclude option exercises
- shares (i64)
- price_per_share (Option<f64>)
- acquisition_or_disposition (char)   // 'A' or 'D'
- total_value (Option<f64>)           // computed

**Top-level `EdgarFundamentals`**:
- ticker (String)
- cik (String)
- company_name (String)
- fetched_at (DateTime<Utc>)
- earnings: Vec<EarningsHistory>      // last 12 quarters
- revenue: Vec<RevenueHistory>        // last 12 quarters
- margins: Vec<MarginHistory>         // last 12 quarters
- cash_flows: Vec<CashFlowHistory>    // last 4 annual periods
- shares: Vec<SharesHistory>          // last 12 quarters
- balance_sheet: Vec<BalanceSheetHistory> // last 8 quarters
- roe: Vec<ReturnOnEquityHistory>     // last 4 annual periods
- insider_transactions: Vec<InsiderTransaction> // last 12 months

### EDGAR API Endpoints to Use

1. **CIK lookup**: `https://efts.sec.gov/LATEST/search-index?q=%22{ticker}%22&dateRange=custom&startdt=2020-01-01&enddt=2025-01-01&forms=10-K`
    - Or better: `https://www.sec.gov/cgi-bin/browse-edgar?company=&CIK={ticker}&type=10-K&dateb=&owner=include&count=10&search_text=&action=getcompany` to resolve ticker → CIK

2. **Company facts (all XBRL financials)**: `https://data.sec.gov/api/xbrl/companyfacts/{CIK}.json`
    - This single endpoint returns all financial data. Parse `facts.us-gaap` for all metrics.
    - Key XBRL concept tags to look for (with fallbacks):
        - EPS diluted: `EarningsPerShareDiluted`
        - EPS basic: `EarningsPerShareBasic`
        - Net income: `NetIncomeLoss`
        - Diluted shares: `WeightedAverageNumberOfDilutedSharesOutstanding`
        - Revenue: try `Revenues`, then `RevenueFromContractWithCustomerExcludingAssessedTax`, then `SalesRevenueNet`
        - Gross profit: `GrossProfit`
        - COGS: `CostOfGoodsAndServicesSold`, fallback `CostOfRevenue`
        - Operating income: `OperatingIncomeLoss`
        - Operating CF: `NetCashProvidedByUsedInOperatingActivities`
        - Capex: `PaymentsToAcquirePropertyPlantAndEquipment`
        - Shares outstanding: `CommonStockSharesOutstanding`
        - Cash: `CashAndCashEquivalentsAtCarryingValue`
        - Long-term debt: `LongTermDebt`
        - Short-term debt: `ShortTermBorrowings`, fallback `LongTermDebtCurrent`
        - Stockholders equity: `StockholdersEquity`

3. **Form 4 insider transactions**:
    - List filings: `https://data.sec.gov/submissions/{CIK}.json` — look at `filings.recent` for form type "4"
    - Parse individual Form 4 XML for transaction details

### Implementation Details

- Use `reqwest` with async/await, `tokio` runtime
- Use `serde` / `serde_json` for JSON parsing
- Add `chrono` for date handling
- Add `anyhow` for error handling
- Respect SEC rate limits: add a 100ms delay between requests, set `User-Agent` header to a valid contact string (required by SEC): `User-Agent: YourAppName your@email.com`
- When parsing companyfacts, filter unit entries to only `USD` for monetary values and `shares` for share counts
- Each XBRL concept has entries with `form`, `start`, `end`, `val`, `accn`, `fy`, `fp` fields. Filter to `form: "10-Q"` for quarterly data and `form: "10-K"` for annual. Use `end` date as the period date.
- Deduplicate entries: if multiple filings cover same period, take the most recent by `accn` (accession number sorts chronologically)
- Sort all vecs by period date ascending before returning
- Compute all derived fields (growth rates, margins, ratios) after raw data is collected
- For EPS acceleration: acceleration[i] = eps_yoy_growth[i] - eps_yoy_growth[i-1]
- For insider transactions: fetch the submission JSON to get a list of Form 4 accession numbers filed in the last 12 months, then fetch and parse each Form 4 XML. Focus on `nonDerivativeTable` transactions. Mark `is_open_market = true` only when `transactionCode == "P"` (open market purchase) or `"S"` (open market sale).

### Module Structure
edgar/
mod.rs          // re-exports
types.rs        // all structs with serde derives + Debug + Clone
client.rs       // EdgarClient struct with reqwest, rate limiting, User-Agent
fetch.rs        // fetch_company_facts(), fetch_insider_transactions()
parse.rs        // parse_companyfacts_json(), parse_form4_xml()
compute.rs      // all derived field calculations
lib.rs          // top-level fetch_fundamentals(ticker: &str) -> Result<EdgarFundamentals>

The main public API should be a single async function:
```rust
pub async fn fetch_fundamentals(ticker: &str) -> Result<EdgarFundamentals>
```

### Additional Notes

- All monetary fields are in USD as reported (do not normalize to millions — keep raw)
- All `Option<f64>` growth/ratio fields should be `None` if insufficient data exists (e.g., first quarter has no YoY)
- The struct should `#[derive(Debug, Clone, Serialize, Deserialize)]` throughout so it can be cached to SQLite or JSON
- Write a basic `main.rs` that calls `fetch_fundamentals("AAPL")` and pretty-prints the result with `{:#?}`
