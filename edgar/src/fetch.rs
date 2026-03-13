use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, warn};

use model::edgar::{Document, InsiderTransaction, InstitutionalHolder};

use crate::{client::EdgarClient, parse};

const MAX_8K: usize = 12;

pub async fn fetch_documents(client: &EdgarClient, ticker: &str) -> anyhow::Result<Vec<Document>> {
    let (cik, _) = client.resolve_cik(ticker).await?;
    let submissions = client.fetch_submissions(&cik).await?;

    let recent = &submissions["filings"]["recent"];
    let empty = vec![];
    let forms = recent["form"].as_array().unwrap_or(&empty);
    let dates = recent["filingDate"].as_array().unwrap_or(&empty);
    let accns = recent["accessionNumber"].as_array().unwrap_or(&empty);
    let primary_docs = recent["primaryDocument"].as_array().unwrap_or(&empty);
    let descriptions = recent["primaryDocDescription"].as_array().unwrap_or(&empty);

    // Collect the latest MAX_8K 8-K filings
    let filings: Vec<(NaiveDate, String, String, String)> = forms
        .iter()
        .zip(dates.iter())
        .zip(accns.iter())
        .zip(primary_docs.iter())
        .zip(descriptions.iter())
        .filter_map(|((((f, d), a), p), desc)| {
            if f.as_str()? != "8-K" {
                return None;
            }
            let date = NaiveDate::parse_from_str(d.as_str()?, "%Y-%m-%d").ok()?;
            Some((
                date,
                a.as_str()?.to_string(),
                p.as_str()?.to_string(),
                desc.as_str().unwrap_or("CURRENT REPORT").to_string(),
            ))
        })
        .take(MAX_8K)
        .collect();

    let raw_cik = cik.trim_start_matches("CIK").trim_start_matches('0');
    let mut documents = Vec::new();

    for (filed_at, accn, primary_doc, description) in filings {
        info!("Fetching 8-K cover page filed {filed_at}: {accn}/{primary_doc}");
        let cover_html = match client.fetch_filing_text(raw_cik, &accn, &primary_doc).await {
            Ok(h) => h,
            Err(e) => {
                warn!("Failed to fetch 8-K {accn}: {e:#}");
                continue;
            }
        };

        let accn_nodash = accn.replace('-', "");
        let filing_base =
            format!("https://www.sec.gov/Archives/edgar/data/{raw_cik}/{accn_nodash}/");

        let cover_page = parse::html_to_markdown(&cover_html);
        let (ex991_file, ex992_file) = parse::extract_exhibit_filenames(&cover_page);

        let press_release =
            fetch_exhibit(client, raw_cik, &accn, ex991_file.as_deref(), &filing_base).await;

        let cfo_commentary =
            fetch_exhibit(client, raw_cik, &accn, ex992_file.as_deref(), &filing_base).await;

        documents.push(Document {
            form_type: "8-K".to_string(),
            filed_at,
            description,
            is_earnings_release: press_release.is_some() || cfo_commentary.is_some(),
            press_release,
            cfo_commentary,
        });
    }

    Ok(documents)
}

/// Fetch a single exhibit file and convert to Markdown. Returns `None` if no
/// filename was provided or the fetch fails.
async fn fetch_exhibit(
    client: &EdgarClient,
    raw_cik: &str,
    accn: &str,
    filename: Option<&str>,
    filing_base: &str,
) -> Option<String> {
    let filename = filename?;
    let url = format!("{filing_base}{filename}");
    info!("Fetching exhibit {url}");
    match client.fetch_filing_text(raw_cik, accn, filename).await {
        Ok(html) => Some(parse::html_to_markdown(&html)),
        Err(e) => {
            warn!("Failed to fetch exhibit {url}: {e:#}");
            None
        }
    }
}

/// Name substrings (uppercase) that identify major institutional investors in form.idx.
/// Used to select which form.idx entries get the directory-listing fallback for info-table
/// filename discovery (needed for filers with non-standard XML filenames like Vanguard).
const MAJOR_INST_PATTERNS: &[&str] = &[
    // Index fund giants
    "VANGUARD",
    "BLACKROCK",
    "BLACK ROCK",
    "STATE STREET",
    "GEODE CAPITAL",
    // Fidelity files as FMR LLC
    "FMR LLC",
    "FIDELITY MANAGEMENT",
    "FIDELITY INVESTMENTS",
    // Capital Group
    "CAPITAL RESEARCH",
    "CAPITAL WORLD",
    // Banks / prime brokers
    "JPMORGAN",
    "JP MORGAN",
    "BANK OF AMERICA",
    "GOLDMAN SACHS",
    "MORGAN STANLEY",
    "CITIGROUP",
    "WELLS FARGO",
    "DEUTSCHE BANK",
    "UBS GROUP",
    // Asset managers
    "NORGES BANK",
    "T. ROWE PRICE",
    "T ROWE PRICE",
    "PRICE T ROWE",
    "WELLINGTON MANAGEMENT",
    "INVESCO",
    "DIMENSIONAL FUND",
    "NORTHERN TRUST",
    "DODGE & COX",
    "CHARLES SCHWAB",
    "COHEN & STEERS",
    "AMERICAN CENTURY",
    "NUVEEN",
    "PRINCIPAL FINANCIAL",
    "PARNASSUS",
    "CAUSEWAY CAPITAL",
    "ARTISAN PARTNERS",
    "BAILIE GIFFORD",
    "BAILLIE GIFFORD",
    "MFS INVESTMENT",
    "COLUMBIA THREADNEEDLE",
    "COLUMBIA MANAGEMENT",
    "NEUBERGER BERMAN",
    "AQR CAPITAL",
    "MANNING & NAPIER",
    // Hedge / quant funds
    "CITADEL ADVISORS",
    "MILLENNIUM MANAGEMENT",
    "JANE STREET",
    "TWO SIGMA",
    "RENAISSANCE",
    "BRIDGEWATER",
    "POINT72",
    "SUSQUEHANNA",
    "DE SHAW",
    "D.E. SHAW",
    "TUDOR INVESTMENT",
    "PERSHING SQUARE",
    "THIRD POINT",
    "GREENLIGHT",
    // Sovereign / pension
    "GOVERNMENT PENSION",
    "CALPERS",
    "CALSTRS",
    "ONTARIO TEACHERS",
    "SINGAPORE",
    "ABU DHABI",
    "SOFTBANK",
];

/// Number of concurrent info-table HTTP fetches.
const PARALLEL_WORKERS: usize = 5;
/// Number of EFTS pages (100 results each) to scan as a supplement.
const EFTS_SUPPLEMENT_PAGES: usize = 10;

/// Fetch 13F-HR institutional holders for a ticker across the last 2 quarters.
///
/// Phase 1: Scan form.idx for 2 quarters to find all known major institutional filers.
///          Uses parallel async fetches; applies directory-listing fallback for major
///          institutions that use non-standard XML filenames.
/// Phase 2: EFTS pagination supplement — adds any additional filers not covered by
///          the form.idx pattern list. EFTS naturally filters to filers that hold the CUSIP.
///
/// Returns one entry per (institution, quarter), sorted newest-first then by shares.
pub async fn fetch_institutional_holders(
    client: &EdgarClient,
    ticker: &str,
) -> anyhow::Result<Vec<InstitutionalHolder>> {
    let (cik, company_name) = client.resolve_cik(ticker).await?;
    let submissions = client.fetch_submissions(&cik).await?;
    let raw_cik = cik.trim_start_matches("CIK").trim_start_matches('0');

    let cusip = match resolve_cusip(client, raw_cik, &company_name, &submissions).await {
        Ok(c) => {
            info!("Resolved CUSIP for {ticker}: {c}");
            c
        }
        Err(e) => {
            warn!("CUSIP strategies 1-2 failed ({e:#}); bootstrapping from 13F info table");
            bootstrap_cusip_from_13f(client, &company_name).await?
        }
    };
    info!("Using CUSIP {cusip} for {ticker}");

    // Look back ~2 quarters (≈7 months to include partial current quarter)
    let today = chrono::Utc::now().date_naive();
    let min_period = today - chrono::Duration::days(200);

    // Phase 1: form.idx (major institutions, with directory-listing fallback)
    let mut holders = fetch_from_quarterly_indices(client, &cusip, 2).await;
    info!(
        "Phase 1 (form.idx): {} institution-quarter entries",
        holders.len()
    );

    // Phase 2: EFTS supplement (smaller/unknown filers, no name filter)
    supplement_with_efts(client, &cusip, min_period, &mut holders).await;
    info!(
        "After EFTS supplement: {} total institution-quarter entries",
        holders.len()
    );

    // Dedup by (institution_name, reported_date), then sort newest-first → shares-desc
    holders.sort_by(|a, b| {
        b.reported_date
            .cmp(&a.reported_date)
            .then(b.shares.cmp(&a.shares))
    });
    let mut seen = std::collections::HashSet::new();
    holders.retain(|h| seen.insert((h.institution_name.clone(), h.reported_date)));
    holders.sort_by(|a, b| {
        b.reported_date
            .cmp(&a.reported_date)
            .then(b.shares.cmp(&a.shares))
    });
    Ok(holders)
}

/// Download form.idx for the last `n_quarters` quarters, collect all matched 13F-HR
/// entries, and fetch each institution's info-table XML in parallel.
async fn fetch_from_quarterly_indices(
    client: &EdgarClient,
    cusip: &str,
    n_quarters: u32,
) -> Vec<InstitutionalHolder> {
    let today = chrono::Utc::now().date_naive();
    let quarters = quarters_back(today, n_quarters);

    // Collect entries from all quarters' form.idx files
    let mut all_entries: Vec<(String, String, String, NaiveDate)> = Vec::new();
    for (year, qtr) in &quarters {
        let url = format!("https://www.sec.gov/Archives/edgar/full-index/{year}/QTR{qtr}/form.idx");
        info!("Downloading form.idx for {year} QTR{qtr}");
        match client.fetch_text(&url).await {
            Ok(text) => {
                let entries = parse_form_index(&text);
                info!("  {} matched entries in {year} QTR{qtr}", entries.len());
                all_entries.extend(entries);
            }
            Err(e) => warn!("Failed to download form.idx for {year} QTR{qtr}: {e:#}"),
        }
    }

    // Parallel fetch using JoinSet + Semaphore
    let sem = Arc::new(Semaphore::new(PARALLEL_WORKERS));
    let mut join_set: JoinSet<Option<InstitutionalHolder>> = JoinSet::new();

    for (name, raw_cik, accn, filed_date) in all_entries {
        let client = client.clone();
        let sem = sem.clone();
        let cusip = cusip.to_string();
        let is_major = MAJOR_INST_PATTERNS
            .iter()
            .any(|p| name.to_uppercase().contains(p));

        join_set.spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let reported_date = infer_period_end(filed_date);
            let xml = if is_major {
                fetch_info_table_xml(&client, &raw_cik, &accn).await.ok()?
            } else {
                fetch_info_table_xml_basic(&client, &raw_cik, &accn)
                    .await
                    .ok()?
            };
            let holder =
                parse::find_holding_in_info_table(&xml, &cusip, true, &name, reported_date);
            if holder.is_some() {
                info!(
                    "{name} ({}): {} shares",
                    reported_date,
                    holder.as_ref().unwrap().shares
                );
            }
            holder
        });
    }

    let mut holders = Vec::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(Some(holder)) = result {
            holders.push(holder);
        }
    }
    holders
}

/// Paginate EDGAR EFTS to find additional 13F-HR filers not covered by the form.idx
/// pattern list. EFTS already filters to filings containing the CUSIP, so no name
/// filter is needed. Appends new entries to `holders`.
async fn supplement_with_efts(
    client: &EdgarClient,
    cusip: &str,
    min_period: NaiveDate,
    holders: &mut Vec<InstitutionalHolder>,
) {
    // Collect already-known (name_upper, period) keys to avoid refetching
    let known: std::collections::HashSet<(String, NaiveDate)> = holders
        .iter()
        .map(|h| (h.institution_name.to_uppercase(), h.reported_date))
        .collect();

    let sem = Arc::new(Semaphore::new(PARALLEL_WORKERS));
    let mut join_set: JoinSet<Option<InstitutionalHolder>> = JoinSet::new();
    let mut any_page_had_recent = false;

    for page in 0..EFTS_SUPPLEMENT_PAGES {
        let from = page * 100;
        let result = match client.search_efts_13f_paged(cusip, from, 100).await {
            Ok(r) => r,
            Err(e) => {
                warn!("EFTS supplement page {page} failed: {e:#}");
                break;
            }
        };

        let empty = vec![];
        let hits = result["hits"]["hits"].as_array().unwrap_or(&empty);
        if hits.is_empty() {
            break;
        }

        let mut page_had_recent = false;
        for hit in hits {
            let src = &hit["_source"];
            let period_str = src["period_ending"].as_str().unwrap_or("");
            let reported_date = match NaiveDate::parse_from_str(period_str, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };
            if reported_date < min_period {
                continue;
            }
            page_had_recent = true;
            any_page_had_recent = true;

            // Extract institution name (strip the " (CIK XXXXXXXXXX)" suffix)
            let name = src["display_names"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .map(|s| {
                    if let Some(idx) = s.rfind("  (CIK") {
                        s[..idx].trim().to_string()
                    } else {
                        s.trim().to_string()
                    }
                })
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            if known.contains(&(name.to_uppercase(), reported_date)) {
                continue;
            }

            let accn = src["adsh"].as_str().unwrap_or("").to_string();
            let filer_cik = src["ciks"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('0').to_string())
                .unwrap_or_default();
            if accn.is_empty() || filer_cik.is_empty() {
                continue;
            }
            let xml_filename = hit["_id"]
                .as_str()
                .and_then(|id| id.split(':').nth(1))
                .unwrap_or("informationTable.xml")
                .to_string();

            let client = client.clone();
            let sem = sem.clone();
            let cusip = cusip.to_string();
            let name_clone = name.clone();

            join_set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                let xml = client
                    .fetch_filing_text(&filer_cik, &accn, &xml_filename)
                    .await
                    .ok()?;
                parse::find_holding_in_info_table(&xml, &cusip, true, &name_clone, reported_date)
            });
        }

        // If this entire page had no results in the target period, stop early
        if !page_had_recent && any_page_had_recent {
            break;
        }
    }

    while let Some(result) = join_set.join_next().await {
        if let Ok(Some(holder)) = result {
            holders.push(holder);
        }
    }
}

/// Parse form.idx (fixed-width space-padded) and return ALL 13F-HR entries.
/// Returns Vec of (institution_name, raw_cik, accession_number, filed_date).
///
/// form.idx layout (space-padded, NOT pipe-delimited):
///   Form Type   Company Name   CIK   Date Filed   File Name
///
/// We parse right-to-left: filename (last token), date (2nd-to-last), CIK (3rd-to-last),
/// then company name is the tokens between form-type and CIK, joined with spaces.
fn parse_form_index(idx: &str) -> Vec<(String, String, String, NaiveDate)> {
    let mut results = Vec::new();
    for line in idx.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        // Need at least: FormType CompanyName CIK Date Filename = 5 tokens minimum
        if tokens.len() < 5 {
            continue;
        }
        let form_type = tokens[0];
        if form_type != "13F-HR" {
            continue;
        }

        // Parse right-to-left: last=filename, second-last=date, third-last=CIK
        let filename = tokens[tokens.len() - 1];
        let date_str = tokens[tokens.len() - 2];
        let cik_str = tokens[tokens.len() - 3];

        // CIK must be all digits
        if !cik_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let filed_date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Company name: tokens[1..len-3] joined with spaces
        let company = tokens[1..tokens.len() - 3].join(" ");

        // Filter to known major institutions — scanning all ~8928 entries per quarter
        // is impractical due to SEC rate limits (~10 req/s).
        let company_upper = company.to_uppercase();
        if !MAJOR_INST_PATTERNS
            .iter()
            .any(|p| company_upper.contains(p))
        {
            continue;
        }

        // Extract accession: "edgar/data/102909/0000102909-26-000031.txt" → "0000102909-26-000031"
        let accn = filename
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".txt")
            .to_string();
        if accn.is_empty() {
            continue;
        }

        results.push((company, cik_str.to_string(), accn, filed_date));
    }
    results
}

/// Return (year, quarter) for the current calendar quarter.
/// e.g. 2026-03-08 → (2026, 1), 2026-07-15 → (2026, 3)
fn current_quarter(today: NaiveDate) -> (i32, u32) {
    let qtr = (today.month() - 1) / 3 + 1;
    (today.year(), qtr)
}

/// Return the `n` most recent (year, quarter) pairs, newest first.
fn quarters_back(today: NaiveDate, n: u32) -> Vec<(i32, u32)> {
    let mut result = Vec::new();
    let (mut year, mut qtr) = current_quarter(today);
    for _ in 0..n {
        result.push((year, qtr));
        if qtr == 1 {
            qtr = 4;
            year -= 1;
        } else {
            qtr -= 1;
        }
    }
    result
}

/// Infer the 13F reporting period end from the filing date.
/// 13F-HR is due 45 days after quarter end. Subtract ~45 days then snap to the
/// nearest quarter-end (Dec 31, Mar 31, Jun 30, Sep 30) by calendar distance.
fn infer_period_end(filed_date: NaiveDate) -> NaiveDate {
    let approx = filed_date - chrono::Duration::days(45);
    let year = approx.year();

    // All plausible quarter-end candidates spanning prev year through next year
    let candidates: [NaiveDate; 6] = [
        NaiveDate::from_ymd_opt(year - 1, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(year, 3, 31).unwrap(),
        NaiveDate::from_ymd_opt(year, 6, 30).unwrap(),
        NaiveDate::from_ymd_opt(year, 9, 30).unwrap(),
        NaiveDate::from_ymd_opt(year, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(year + 1, 3, 31).unwrap(),
    ];

    candidates
        .into_iter()
        .min_by_key(|d| (*d - approx).num_days().unsigned_abs())
        .unwrap_or(approx)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Fetch info table XML using only the two standard filenames (no directory listing).
/// Used for non-major institutions where a custom filename is unlikely.
async fn fetch_info_table_xml_basic(
    client: &EdgarClient,
    raw_cik: &str,
    accn: &str,
) -> anyhow::Result<String> {
    for name in &["informationTable.xml", "infotable.xml"] {
        if let Ok(xml) = client.fetch_filing_text(raw_cik, accn, name).await {
            if xml.contains("infoTable") || xml.contains("InformationTable") {
                return Ok(xml);
            }
        }
    }
    anyhow::bail!("Info table XML not found for CIK={raw_cik} accn={accn}")
}

/// Fetch a 13F-HR information table XML for the given filer.
/// Tries standard filenames first, then falls back to parsing the directory index.
async fn fetch_info_table_xml(
    client: &EdgarClient,
    raw_cik: &str,
    accn: &str,
) -> anyhow::Result<String> {
    // Try the two most common standard filenames first
    for name in &["informationTable.xml", "infotable.xml"] {
        if let Ok(xml) = client.fetch_filing_text(raw_cik, accn, name).await {
            if xml.contains("infoTable") || xml.contains("InformationTable") {
                return Ok(xml);
            }
        }
    }

    // Fallback: fetch the filing's HTML directory index and scan for XML hrefs
    let accn_nodash = accn.replace('-', "");
    let dir_url = format!("https://www.sec.gov/Archives/edgar/data/{raw_cik}/{accn_nodash}/");
    let dir_html = client.fetch_text(&dir_url).await?;

    // Extract .xml hrefs from the directory listing HTML
    for href in extract_xml_hrefs_from_dir(&dir_html) {
        if let Ok(xml) = client.fetch_filing_text(raw_cik, accn, &href).await {
            if xml.contains("infoTable") || xml.contains("InformationTable") {
                return Ok(xml);
            }
        }
    }

    anyhow::bail!("Info table XML not found for CIK={raw_cik} accn={accn}")
}

/// Extract .xml filenames from an EDGAR directory listing HTML page.
/// The directory listing uses absolute hrefs like `/Archives/edgar/data/{cik}/{accn}/{file}.xml`.
/// Returns just the bare filenames (last path segment), skipping `primary_doc.xml`.
fn extract_xml_hrefs_from_dir(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(href_start) = html[pos..].find("href=\"") {
        let abs = pos + href_start + 6;
        let Some(end) = html[abs..].find('"') else {
            break;
        };
        let href = &html[abs..abs + end];
        let lower = href.to_lowercase();
        if lower.ends_with(".xml") {
            // Extract bare filename from absolute path
            let filename = href.rsplit('/').next().unwrap_or(href);
            if filename.to_lowercase() != "primary_doc.xml" {
                out.push(filename.to_string());
            }
        }
        pos = abs + end + 1;
    }
    out
}

/// Bootstrap CUSIP by doing a name-based EFTS search and parsing the first
/// matching 13F info-table XML to extract the CUSIP from a `nameOfIssuer` row.
/// Tries up to 3 pages of results (300 total) to find a match.
///
/// Uses `_id` field from EFTS (which encodes the exact indexed filename) to
/// avoid expensive directory-listing requests. Falls back to standard filenames
/// if `_id` doesn't yield a `.xml` name.
async fn bootstrap_cusip_from_13f(
    client: &EdgarClient,
    company_name: &str,
) -> anyhow::Result<String> {
    for page in 0..3usize {
        let efts = client
            .search_efts_13f_paged(company_name, page * 100, 100)
            .await?;
        let hits = efts["hits"]["hits"].as_array().cloned().unwrap_or_default();
        if hits.is_empty() {
            break;
        }
        info!(
            "Bootstrap page {page}: {} EFTS hits for '{company_name}'",
            hits.len()
        );

        for hit in &hits {
            let adsh = hit["_source"]["adsh"].as_str().unwrap_or("").to_string();
            let filer_cik = hit["_source"]["ciks"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('0').to_string())
                .unwrap_or_default();
            if filer_cik.is_empty() || adsh.is_empty() {
                continue;
            }

            // Candidate filenames: EFTS _id encodes the indexed filename as "accn:filename"
            // Try that first, then fall back to the two standard names.
            let id_filename = hit["_id"]
                .as_str()
                .and_then(|id| id.split(':').nth(1))
                .filter(|f| {
                    let fl = f.to_lowercase();
                    fl.ends_with(".xml") && fl != "primary_doc.xml"
                })
                .map(|s| s.to_string());

            let candidates: Vec<&str> = {
                let mut v: Vec<&str> = Vec::new();
                if let Some(ref f) = id_filename {
                    v.push(f.as_str());
                }
                v.push("informationTable.xml");
                v.push("infotable.xml");
                v
            };

            let mut found = false;
            for filename in candidates {
                match client.fetch_filing_text(&filer_cik, &adsh, filename).await {
                    Ok(xml) if xml.contains("infoTable") || xml.contains("InformationTable") => {
                        if let Some(cusip) =
                            parse::extract_cusip_from_info_table(&xml, company_name)
                        {
                            info!(
                                "Bootstrapped CUSIP {cusip} for '{company_name}' from 13F {adsh}"
                            );
                            return Ok(cusip);
                        }
                        // XML fetched but CUSIP not found — no point trying other filenames
                        found = true;
                        break;
                    }
                    _ => continue,
                }
            }
            let _ = found; // silence unused warning
        }
    }
    anyhow::bail!("Could not bootstrap CUSIP for '{company_name}' from 13F info tables")
}

/// Resolve the CUSIP for a company via two strategies:
/// 1. Extract `EntityCUSIP` from the recent 10-K primary document (iXBRL HTML).
/// 2. Search EDGAR EFTS for older plain-XBRL 10-K instance documents (`.xml`) that
///    contain `EntityCUSIP` as a plain XML tag, then extract it from there.
async fn resolve_cusip(
    client: &EdgarClient,
    raw_cik: &str,
    company_name: &str,
    submissions: &serde_json::Value,
) -> anyhow::Result<String> {
    let recent = &submissions["filings"]["recent"];
    let empty = vec![];
    let forms = recent["form"].as_array().unwrap_or(&empty);
    let accns = recent["accessionNumber"].as_array().unwrap_or(&empty);
    let primary_docs = recent["primaryDocument"].as_array().unwrap_or(&empty);

    // Strategy 1: scan recent 10-K primary docs for EntityCUSIP
    for ((f, a), p) in forms.iter().zip(accns.iter()).zip(primary_docs.iter()) {
        if f.as_str() != Some("10-K") {
            continue;
        }
        let accn = a.as_str().unwrap_or("");
        let doc = p.as_str().unwrap_or("");
        let html = match client.fetch_filing_text(raw_cik, accn, doc).await {
            Ok(h) => h,
            Err(_) => continue,
        };
        if let Some(cusip) = parse::extract_cusip(&html) {
            return Ok(cusip);
        }
        // Only check the most recent 10-K via this path
        break;
    }

    // Strategy 2: search EFTS for old plain-XBRL 10-K instance documents (2013–2021)
    // that contain "EntityCUSIP" along with the company name
    let efts = client
        .search_efts_any(
            &format!("EntityCUSIP {company_name}"),
            "10-K",
            "2013-01-01",
            "2021-12-31",
            5,
        )
        .await?;

    let hits = efts["hits"]["hits"].as_array().cloned().unwrap_or_default();
    for hit in &hits {
        let hit_id = hit["_id"].as_str().unwrap_or("");
        let xml_filename = match hit_id.split(':').nth(1) {
            Some(f) if f.ends_with(".xml") => f,
            _ => continue,
        };
        let adsh = hit["_source"]["adsh"].as_str().unwrap_or("");
        let filer_cik = hit["_source"]["ciks"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| s.trim_start_matches('0'))
            .unwrap_or(raw_cik);
        let xml = match client
            .fetch_filing_text(filer_cik, adsh, xml_filename)
            .await
        {
            Ok(x) => x,
            Err(_) => continue,
        };
        if let Some(cusip) = parse::extract_cusip(&xml) {
            return Ok(cusip);
        }
    }

    anyhow::bail!("EntityCUSIP not found for {company_name}")
}

pub async fn fetch_insider_transactions(
    client: &EdgarClient,
    ticker: &str,
) -> anyhow::Result<Vec<InsiderTransaction>> {
    let (cik, _) = client.resolve_cik(ticker).await?;
    let submissions = client.fetch_submissions(&cik).await?;

    let recent = &submissions["filings"]["recent"];
    let empty = vec![];
    let forms = recent["form"].as_array().unwrap_or(&empty);
    let dates = recent["filingDate"].as_array().unwrap_or(&empty);
    let accns = recent["accessionNumber"].as_array().unwrap_or(&empty);
    let primary_docs = recent["primaryDocument"].as_array().unwrap_or(&empty);

    let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(365);

    let mut form4_filings: Vec<(NaiveDate, String, String)> = forms
        .iter()
        .zip(dates.iter())
        .zip(accns.iter())
        .zip(primary_docs.iter())
        .filter_map(|(((f, d), a), p)| {
            if f.as_str()? != "4" {
                return None;
            }
            let date = NaiveDate::parse_from_str(d.as_str()?, "%Y-%m-%d").ok()?;
            if date < cutoff {
                return None;
            }
            Some((date, a.as_str()?.to_string(), p.as_str()?.to_string()))
        })
        .collect();

    form4_filings.sort_by_key(|(d, _, _)| *d);

    let raw_cik = cik.trim_start_matches("CIK").trim_start_matches('0');
    let mut all_txs: Vec<InsiderTransaction> = vec![];

    for (_date, accn, primary_doc) in &form4_filings {
        let xml = match client.fetch_filing_text(raw_cik, accn, primary_doc).await {
            Ok(x) => x,
            Err(e) => {
                warn!("Skipping Form 4 {accn}: {e:#}");
                continue;
            }
        };

        match parse::parse_form4_xml(&xml) {
            Ok(raw_txs) => {
                for tx in raw_txs {
                    let is_open_market = tx.code == "P" || tx.code == "S";
                    if !is_open_market || tx.acq_disp != 'A' {
                        continue;
                    }
                    let total_value = tx.price.map(|p| tx.shares * p);
                    all_txs.push(InsiderTransaction {
                        transaction_date: tx.date,
                        insider_name: tx.insider_name,
                        insider_role: tx.insider_role,
                        is_open_market,
                        shares: tx.shares as i64,
                        price_per_share: tx.price,
                        acquisition_or_disposition: tx.acq_disp,
                        total_value,
                    });
                }
            }
            Err(e) => warn!("Failed to parse Form 4 {accn}: {e:#}"),
        }
    }

    all_txs.sort_by_key(|t| t.transaction_date);
    Ok(all_txs)
}
