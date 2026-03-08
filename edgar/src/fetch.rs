use chrono::NaiveDate;
use tracing::{info, warn};

use model::edgar::{Document, InsiderTransaction};

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
            cover_page: Some(cover_page),
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
