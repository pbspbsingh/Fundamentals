//! Parse raw SEC EDGAR JSON and Form 4 XML into intermediate vectors of
//! `(period_end: NaiveDate, fy: u16, fp: String, val: T)` tuples.

use anyhow::Context;
use chrono::NaiveDate;

/// A single XBRL fact entry after filtering.
#[derive(Debug, Clone)]
pub struct Fact {
    pub end: NaiveDate,
    pub fy: u16,
    /// "Q1".."Q4" or "FY" / "CY"
    pub fp: String,
    pub val: f64,
    /// Accession number — used to deduplicate (higher = more recent)
    pub accn: String,
}

/// A parsed Form 4 non-derivative transaction.
#[derive(Debug, Clone)]
pub struct RawInsiderTx {
    pub date: NaiveDate,
    pub insider_name: String,
    pub insider_role: String,
    pub code: String,
    pub shares: f64,
    pub price: Option<f64>,
    pub acq_disp: char,
}

// ── Company facts ────────────────────────────────────────────────────────────

/// Extract quarterly (`form = "10-Q"`) USD facts, merging all fallback concepts.
/// Earlier concepts take priority for the same period; later ones fill gaps.
pub fn extract_quarterly(facts: &serde_json::Value, concepts: &[&str]) -> Vec<Fact> {
    merge_concepts(facts, concepts, "USD", &["10-Q"])
}

/// Extract annual (`form = "10-K"`) USD facts, merging all fallback concepts.
pub fn extract_annual(facts: &serde_json::Value, concepts: &[&str]) -> Vec<Fact> {
    merge_concepts(facts, concepts, "USD", &["10-K"])
}

/// Extract quarterly share-count facts (unit = "shares"), merging fallback concepts.
pub fn extract_quarterly_shares(facts: &serde_json::Value, concepts: &[&str]) -> Vec<Fact> {
    merge_concepts(facts, concepts, "shares", &["10-Q", "10-K"])
}

/// Extract quarterly EPS facts (unit = "USD/shares"), merging fallback concepts.
pub fn extract_quarterly_eps(facts: &serde_json::Value, concepts: &[&str]) -> Vec<Fact> {
    merge_concepts(facts, concepts, "USD/shares", &["10-Q"])
}

/// Merge facts from multiple concepts. The first concept to provide a value for
/// a given period wins; later concepts only fill dates that have no entry yet.
fn merge_concepts(
    facts: &serde_json::Value,
    concepts: &[&str],
    unit: &str,
    forms: &[&str],
) -> Vec<Fact> {
    let mut map: std::collections::HashMap<NaiveDate, Fact> =
        std::collections::HashMap::new();
    for concept in concepts {
        for fact in extract_facts(facts, concept, unit, forms) {
            map.entry(fact.end).or_insert(fact);
        }
    }
    map.into_values().collect()
}

fn extract_facts(
    facts: &serde_json::Value,
    concept: &str,
    unit: &str,
    forms: &[&str],
) -> Vec<Fact> {
    let entries = facts
        .pointer(&format!("/us-gaap/{concept}/units/{unit}"))
        .and_then(|v| v.as_array());

    let Some(arr) = entries else {
        return vec![];
    };

    // Collect, filter to desired forms, deduplicate by `end` keeping latest accn
    let mut map: std::collections::HashMap<NaiveDate, Fact> =
        std::collections::HashMap::new();

    for entry in arr {
        let form = entry["form"].as_str().unwrap_or("");
        if !forms.contains(&form) {
            continue;
        }
        // EDGAR files both a single-quarter figure and a YTD cumulative under the same
        // end date and fp for 10-Q filings. The single-quarter entry always carries a
        // `frame` field (e.g. "CY2025Q1"); YTD/cumulative entries don't. Filter by that.
        // For 10-K, use duration (300–400 days) to accept full-year entries only.
        let is_10k = form == "10-K";
        if is_10k {
            if let (Some(start_str), Some(end_str)) =
                (entry["start"].as_str(), entry["end"].as_str())
            {
                let start = NaiveDate::parse_from_str(start_str, "%Y-%m-%d");
                let end = NaiveDate::parse_from_str(end_str, "%Y-%m-%d");
                if let (Ok(s), Ok(e)) = (start, end) {
                    if !(300..=400).contains(&(e - s).num_days()) {
                        continue;
                    }
                }
            }
        } else {
            // 10-Q: accept single-period entries.
            // Modern filings (post ~2021) carry a `frame` field; older filings don't.
            // Accept if: has a frame field  OR  duration ≤ 120 days (true quarterly, not YTD).
            let has_frame = entry.get("frame").map(|v| !v.is_null()).unwrap_or(false);
            if !has_frame {
                // Fall back to duration check
                let within_quarter = match (entry["start"].as_str(), entry["end"].as_str()) {
                    (Some(s), Some(e)) => {
                        match (
                            NaiveDate::parse_from_str(s, "%Y-%m-%d"),
                            NaiveDate::parse_from_str(e, "%Y-%m-%d"),
                        ) {
                            (Ok(sd), Ok(ed)) => (ed - sd).num_days() <= 120,
                            _ => false,
                        }
                    }
                    // Instant facts (no start) are balance-sheet items — allow them
                    (None, Some(_)) => true,
                    _ => false,
                };
                if !within_quarter {
                    continue;
                }
            }
        }

        let end_str = entry["end"].as_str().unwrap_or("");
        let Ok(end) = NaiveDate::parse_from_str(end_str, "%Y-%m-%d") else {
            continue;
        };
        let val = match &entry["val"] {
            serde_json::Value::Number(n) => match n.as_f64() {
                Some(f) => f,
                None => continue,
            },
            _ => continue,
        };
        let fy = entry["fy"].as_u64().unwrap_or(0) as u16;
        let fp = entry["fp"].as_str().unwrap_or("").to_string();
        let accn = entry["accn"].as_str().unwrap_or("").to_string();

        let existing = map.entry(end).or_insert_with(|| Fact {
            end,
            fy,
            fp: fp.clone(),
            val,
            accn: accn.clone(),
        });

        // Keep the entry with the lexicographically greater accession number (more recent)
        if accn > existing.accn {
            *existing = Fact { end, fy, fp, val, accn };
        }
    }

    map.into_values().collect()
}

// ── Form 4 XML ───────────────────────────────────────────────────────────────

/// Parse Form 4 XML text. Returns all non-derivative transactions found.
pub fn parse_form4_xml(xml: &str) -> anyhow::Result<Vec<RawInsiderTx>> {
    // Form 4 SGML/XML has a mixed header; find the start of XML
    let xml_start = xml.find("<?xml").or_else(|| xml.find("<XML>")).unwrap_or(0);
    let xml_body = &xml[xml_start..];

    // Strip <XML> / </XML> SGML wrapper if present
    let xml_body = xml_body
        .trim_start_matches("<XML>")
        .trim_end_matches("</XML>")
        .trim();

    let doc = roxmltree::Document::parse(xml_body)
        .context("parsing Form 4 XML")?;

    let root = doc.root_element();

    // Reporting owner info
    let insider_name = find_text(&root, "rptOwnerName").unwrap_or_default();
    let is_officer = find_text(&root, "isOfficer")
        .map(|s| s.trim() == "1")
        .unwrap_or(false);
    let is_director = find_text(&root, "isDirector")
        .map(|s| s.trim() == "1")
        .unwrap_or(false);
    let is_ten_pct = find_text(&root, "isTenPercentOwner")
        .map(|s| s.trim() == "1")
        .unwrap_or(false);
    let officer_title = find_text(&root, "officerTitle").unwrap_or_default();

    let insider_role = if is_officer && !officer_title.is_empty() {
        officer_title
    } else if is_officer {
        "Officer".to_string()
    } else if is_director {
        "Director".to_string()
    } else if is_ten_pct {
        "10% Owner".to_string()
    } else {
        "Other".to_string()
    };

    let mut txs = vec![];

    // Iterate nonDerivativeTransaction nodes
    for node in root.descendants() {
        if node.tag_name().name() != "nonDerivativeTransaction" {
            continue;
        }

        let date_str = find_child_text(&node, "transactionDate")
            .or_else(|| find_child_text(&node, "transactionDateValue"))
            .unwrap_or_default();
        let Ok(date) = NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d") else {
            continue;
        };

        // transactionCode is nested inside <transactionCoding>
        let code = find_child_text(&node, "transactionCode").unwrap_or_default();

        let shares = find_child_text(&node, "transactionShares")
            .or_else(|| find_child_text(&node, "transactionSharesValue"))
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        let price = find_child_text(&node, "transactionPricePerShare")
            .or_else(|| find_child_text(&node, "transactionPricePerShareValue"))
            .and_then(|s| s.trim().parse::<f64>().ok());

        let acq_disp = find_child_text(&node, "transactionAcquiredDisposedCode")
            .or_else(|| find_child_text(&node, "transactionAcquiredDisposedCodeValue"))
            .and_then(|s| s.trim().chars().next())
            .unwrap_or('A');

        txs.push(RawInsiderTx {
            date,
            insider_name: insider_name.clone(),
            insider_role: insider_role.clone(),
            code,
            shares,
            price,
            acq_disp,
        });
    }

    Ok(txs)
}

fn find_text(root: &roxmltree::Node, tag: &str) -> Option<String> {
    root.descendants()
        .find(|n| n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(|s| s.to_string())
}

fn find_child_text(node: &roxmltree::Node, tag: &str) -> Option<String> {
    let found = node.descendants().find(|n| n.tag_name().name() == tag)?;
    // Form 4 XML wraps most values in a nested <value> element; fall back to direct text
    found
        .children()
        .find(|n| n.tag_name().name() == "value")
        .and_then(|n| n.text())
        .or_else(|| found.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
