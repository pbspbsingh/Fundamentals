use chrono::NaiveDate;

#[inline]
pub(super) fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Parse a TradingView numeric value like "634.34 M", "1.63 B", "-105.53 M".
/// Unicode directional marks (U+202A/202C) are stripped; U+2212 is treated as minus.
/// Returns `None` for TradingView's "no data" sentinel "—" (U+2014) and empty strings.
pub(super) fn parse_value(s: &str) -> Option<f64> {
    let clean: String = s
        .chars()
        .filter(|&c| {
            !matches!(
                c,
                '\u{202A}' | '\u{202B}' | '\u{202C}' | '\u{200E}' | '\u{200F}'
            )
        })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() || clean == "\u{2014}" {
        return None;
    }

    let (neg, rest) = if clean.starts_with('\u{2212}') || clean.starts_with('-') {
        let skip = clean.chars().next()?.len_utf8();
        (true, &clean[skip..])
    } else {
        (false, clean)
    };

    // Detect suffix by the last character; trim_end() removes whatever separator
    // precedes it (may be U+00A0 non-breaking space, not a regular ASCII space).
    let (num_str, mult) = match rest.chars().last() {
        Some('T') => (rest[..rest.len() - 'T'.len_utf8()].trim_end(), 1e12_f64),
        Some('B') => (rest[..rest.len() - 'B'.len_utf8()].trim_end(), 1e9_f64),
        Some('M') => (rest[..rest.len() - 'M'.len_utf8()].trim_end(), 1e6_f64),
        Some('K') => (rest[..rest.len() - 'K'.len_utf8()].trim_end(), 1e3_f64),
        _ => (rest, 1.0_f64),
    };

    let n: f64 = num_str.trim().parse().ok()?;
    Some(round3(if neg { -(n * mult) } else { n * mult }))
}

/// Parse a TradingView percentage string like "+20.78%" or "−15.40%" → fractional f64.
/// Returns `None` for TradingView's "no data" sentinel "—" (U+2014) and empty strings.
pub(super) fn parse_pct(s: &str) -> Option<f64> {
    let clean: String = s
        .chars()
        .filter(|&c| {
            !matches!(
                c,
                '\u{202A}' | '\u{202B}' | '\u{202C}' | '\u{200E}' | '\u{200F}'
            )
        })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() || clean == "\u{2014}" {
        return None;
    }
    // Replace unicode minus sign with ASCII minus, then strip leading '+' and trailing '%'
    let clean = clean.replace('\u{2212}', "-");
    let n: f64 = clean.trim_start_matches('+').trim_end_matches('%').parse().ok()?;
    // Round to 5 decimal places (= 3 decimal places of the percentage).
    // pct_serde serializes as a string via format!("{:.3}", v * 100), so floating
    // point noise never reaches the JSON regardless, but we round here for clean
    // in-memory values.
    Some((n / 100.0 * 1e5).round() / 1e5)
}

/// Parse an earnings column label to the last day of that fiscal period.
///
/// Supported formats:
/// - `"Q1 '21"` → 2021-03-31  (Q1→Mar, Q2→Jun, Q3→Sep, Q4→Dec)
/// - `"FY '21"` or `"2021"` → 2021-12-31
///
/// Note: these are calendar-quarter end dates used as approximations; actual
/// fiscal period ends may differ by company.
pub(super) fn parse_earnings_label(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    let mut parts = s.split_whitespace();
    let first = parts.next()?;
    let second = parts.next();

    if let Some(q) = first.strip_prefix('Q') {
        // Quarterly: "Q1 '21"
        let quarter: u32 = q.parse().ok()?;
        let y_str = second?.trim_start_matches('\'');
        let year_short: i32 = y_str.parse().ok()?;
        let year = 2000 + year_short;
        let month = match quarter {
            1 => 3,
            2 => 6,
            3 => 9,
            4 => 12,
            _ => return None,
        };
        last_day_of_month(year, month)
    } else if first == "FY" {
        // Annual: "FY '21"
        let y_str = second?.trim_start_matches('\'');
        let year_short: i32 = y_str.parse().ok()?;
        last_day_of_month(2000 + year_short, 12)
    } else {
        // Plain year: "2021"
        let year: i32 = first.parse().ok()?;
        last_day_of_month(year, 12)
    }
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let first_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    first_next.pred_opt()
}

/// Parse "Mar 2019" → last day of that month (2019-03-31).
pub(super) fn parse_month_year(s: &str) -> Option<NaiveDate> {
    let mut parts = s.split_whitespace();
    let month: u32 = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i32 = parts.next()?.parse().ok()?;
    last_day_of_month(year, month)
}
