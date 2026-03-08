use anyhow::Context;
use chrono::NaiveDate;

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
