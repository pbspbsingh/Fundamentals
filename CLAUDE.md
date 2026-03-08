# Fundamentals

A Rust workspace that scrapes TradingView financial data via Chrome automation, suitable for
CANSLIM, SEPA, and Episodic Pivot analysis.

## Workspace Structure

- `runner/` — binary; takes ticker as CLI arg, prints `TradingViewFinancials` JSON to stdout
- `scraper/` — TradingView Chrome scraper (chromiumoxide); fully implemented
- `model/` — shared types; `model/src/financials.rs` defines all structs
- `config/` — `config.toml` singleton via `OnceLock`

## Build & Run

```bash
# from workspace root (config.toml must be present here)
cargo build --release
cargo run --release -- AAPL          # stdout = JSON, stderr = logs
RUST_LOG=debug cargo run -- AAPL    # verbose
```

## Key Files

| File | Purpose |
|------|---------|
| `scraper/src/financial_scraper/mod.rs` | `FinancialScraper` struct, `fetch_financials`, tab navigation, JS evaluation |
| `scraper/src/financial_scraper/parse.rs` | `impl FinancialScraper` — all `parse_*` methods |
| `scraper/src/financial_scraper/table.rs` | `TableData`, `collect_entries<T: HasPeriodEnd>`, `find_ttm_col` |
| `scraper/src/financial_scraper/utils.rs` | `parse_value`, `parse_pct`, `parse_month_year`, `parse_earnings_label` |
| `model/src/financials.rs` | All structs with serde derives (`TradingViewFinancials`, entries, `pct_serde`) |
| `runner/src/main.rs` | Tracing setup, calls scraper, prints JSON |

## Implementation Notes

### Period labels
`Period.label` is always scraped directly from TradingView — never derived. For financial
statement tables it comes from `col["label"]` in the JS output. For earnings it comes from
the DOM label text. For TTM it comes from `find_ttm_col` which reads the TTM column label.

### TradingView DOM — Financial Statement Tables
- Rows identified by `data-name` attribute
- Date cells use `subvalue-*` class prefix
- Column labels come from JS-extracted `col["label"]`
- TTM column: `col["date"]` is `null`; `period_end` = max date among all non-null columns

### TradingView DOM — Earnings Tables
Different structure from financial statement tables:
- Rows: `container-C9MdAMrq`, labels: `titleText-C9MdAMrq`, values: `values-C9MdAMrq`
- Separate JS extractor: `evaluate_earnings_js()`
- EPS and Revenue each have independent tab bars; click all simultaneously:
  `document.querySelectorAll('[id="FQ"]').forEach(b => b.click())`

### pct_serde
`_yoy` and `_surprise` fields use a custom serde module:
- Serializes `Option<f64>` fraction as percentage string: `0.0352` → `"3.52%"`
- Deserializes from percentage string or raw float (back-compat)
- `parse_pct` uses 5-decimal rounding (`round5`) to avoid truncation; `pct_serde` serializes
  via `format!("{:.3}", v * 100)` which clamps any remaining float noise

### Deps gotchas
- **reqwest 0.13**: use `{ version = "0.13", features = ["json"] }` with default features.
  Do not set `default-features = false` — TLS feature names changed in 0.13.
- **tracing**: add `.with_writer(std::io::stderr)` to the subscriber so stdout stays clean JSON.
