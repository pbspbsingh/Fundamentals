use anyhow::Context;
use chrono::NaiveDate;
use model::edgar::InstitutionalHolder;

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

    let doc = roxmltree::Document::parse(xml_body).context("parsing Form 4 XML")?;
    let root = doc.root_element();

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

/// Scan the Markdown-rendered cover page for Exhibit 99.1 / 99.2 filenames.
///
/// `htmd` converts `<a href="q4fy26pr.htm">…</a>` to `[…](q4fy26pr.htm)`, so
/// after rendering we just need to find the last occurrence of "99.1" / "99.2"
/// (last = exhibit table at the end, not the first body-text reference) and
/// then grab the first `](*.htm)` link that follows within 600 chars.
pub fn extract_exhibit_filenames(markdown: &str) -> (Option<String>, Option<String>) {
    let ex991 = find_exhibit_filename(markdown, "99.1");
    let ex992 = find_exhibit_filename(markdown, "99.2");
    (ex991, ex992)
}

fn find_exhibit_filename(markdown: &str, marker: &str) -> Option<String> {
    // Use rfind so we land in the exhibit table (end of doc), not the first
    // body-text mention like "attached as Exhibit 99.1".
    let pos = markdown.rfind(marker)?;
    // Collect by chars (not bytes) to avoid slicing inside a multi-byte codepoint.
    let window: String = markdown[pos..].chars().take(600).collect();
    let link_start = window.find("](")?;
    let after = &window[link_start + 2..];
    let end = after.find(')')?;
    let href = &after[..end];
    let lower = href.to_lowercase();
    if lower.ends_with(".htm") || lower.ends_with(".html") {
        Some(href.to_string())
    } else {
        None
    }
}

/// Convert an HTML filing to Markdown, falling back to plain-text tag stripping on failure.
pub fn html_to_markdown(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| {
        // Fallback: naive tag stripper
        let mut out = String::new();
        let mut in_tag = false;
        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    })
}

/// Extract the 9-character CUSIP from an SEC filing document (10-K inline XBRL or plain HTML).
///
/// EDGAR 10-K inline XBRL files contain a tag like:
/// `<ix:nonNumeric name="dei:EntityCUSIP" ...>67066G104</ix:nonNumeric>`
/// We find the `EntityCUSIP` marker then grab the first 9-character alphanumeric run after it.
pub fn extract_cusip(html: &str) -> Option<String> {
    let pos = html.find("EntityCUSIP")?;
    let after = &html[pos + "EntityCUSIP".len()..];
    // Scan up to 500 bytes ahead for the first 9-char alphanumeric run (the CUSIP itself).
    // Tag attribute values (contextRef, id, etc.) are typically shorter or longer than 9 chars.
    let window = &after[..after.len().min(500)];
    let mut run_start: Option<usize> = None;
    let mut run_len = 0usize;
    for (i, b) in window.bytes().enumerate() {
        if b.is_ascii_alphanumeric() {
            if run_start.is_none() {
                run_start = Some(i);
            }
            run_len += 1;
        } else {
            if run_len == 9 {
                let s = run_start.unwrap();
                return Some(window[s..s + 9].to_uppercase());
            }
            run_start = None;
            run_len = 0;
        }
    }
    if run_len == 9 {
        let s = run_start.unwrap();
        return Some(window[s..s + 9].to_uppercase());
    }
    None
}

/// Parse an information table XML from a 13F-HR filing and return the holding for the
/// target company, if present.
///
/// When `match_by_cusip` is `true`, `match_key` is compared against the row's `<cusip>` field
/// (exact, case-insensitive).  When `false`, it is checked as a substring of `<nameOfIssuer>`
/// (case-insensitive) — used as a fallback when the CUSIP is unknown.
///
/// Shares/value are summed across multiple rows with the same key (some filers split entries).
pub fn find_holding_in_info_table(
    xml: &str,
    match_key: &str,
    match_by_cusip: bool,
    institution_name: &str,
    reported_date: NaiveDate,
) -> Option<InstitutionalHolder> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();

    let key_upper = match_key.to_uppercase();
    let mut total_shares: i64 = 0;
    let mut total_value: i64 = 0;
    let mut found = false;

    for node in root.descendants() {
        if !node.tag_name().name().eq_ignore_ascii_case("infoTable") {
            continue;
        }

        let row_matches = if match_by_cusip {
            node.descendants()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("cusip"))
                .and_then(|n| n.text())
                .map(|s| s.trim().to_uppercase() == key_upper)
                .unwrap_or(false)
        } else {
            node.descendants()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("nameOfIssuer"))
                .and_then(|n| n.text())
                .map(|s| s.to_uppercase().contains(&key_upper))
                .unwrap_or(false)
        };

        if !row_matches {
            continue;
        }
        found = true;

        let shares: i64 = node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("sshPrnamt"))
            .and_then(|n| n.text())
            .and_then(|s| s.trim().replace(',', "").parse::<i64>().ok())
            .unwrap_or(0);

        let value: i64 = node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("value"))
            .and_then(|n| n.text())
            .and_then(|s| s.trim().replace(',', "").parse::<i64>().ok())
            .unwrap_or(0);

        total_shares += shares;
        total_value += value;
    }

    if found && total_shares > 0 {
        Some(InstitutionalHolder {
            institution_name: institution_name.to_string(),
            shares: total_shares,
            market_value_usd: total_value,
            reported_date,
        })
    } else {
        None
    }
}

/// Scan a 13F information table XML and return the CUSIP of the first row whose
/// `nameOfIssuer` contains `company_name_keyword` (case-insensitive substring match).
///
/// Used to bootstrap a CUSIP when it cannot be found in the company's own SEC filings.
pub fn extract_cusip_from_info_table(xml: &str, company_name_keyword: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    let keyword = company_name_keyword.to_uppercase();
    // Use first word of name for flexible matching ("NVIDIA" matches "NVIDIA CORP" etc.)
    let first_word: &str = keyword.split_whitespace().next().unwrap_or(&keyword);

    for node in root.descendants() {
        if !node.tag_name().name().eq_ignore_ascii_case("infoTable") {
            continue;
        }
        let issuer = node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("nameOfIssuer"))
            .and_then(|n| n.text())
            .map(|s| s.to_uppercase())
            .unwrap_or_default();
        if !issuer.contains(first_word) {
            continue;
        }
        if let Some(cusip) = node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("cusip"))
            .and_then(|n| n.text())
            .map(|s| s.trim().to_uppercase())
        {
            if cusip.len() == 9 {
                return Some(cusip);
            }
        }
    }
    None
}

fn find_text(root: &roxmltree::Node, tag: &str) -> Option<String> {
    root.descendants()
        .find(|n| n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(|s| s.to_string())
}

fn find_child_text(node: &roxmltree::Node, tag: &str) -> Option<String> {
    let found = node.descendants().find(|n| n.tag_name().name() == tag)?;
    found
        .children()
        .find(|n| n.tag_name().name() == "value")
        .and_then(|n| n.text())
        .or_else(|| found.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
