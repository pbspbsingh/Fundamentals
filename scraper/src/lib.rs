use chrome_driver::{Browser, ChromeDriverConfig, Page, Sleepable};
use chrono::NaiveDate;
use model::{Fundamentals, Ticker};

pub async fn start_fetching(ticker: &Ticker) -> anyhow::Result<Fundamentals> {
    let browser = launch_browser().await?;
    let page = browser
        .new_page(format!(
            "https://www.tradingview.com/symbols/{}-{}/",
            ticker.exchange, ticker.ticker
        ))
        .await?;

    // Initial wait for the page JS to execute
    page.sleep().await;

    let description = extract_description(&page).await?;
    let earnings_date_str = extract_labeled_value(&page, "Last report date").await?;
    let ipo_date_str = extract_labeled_value(&page, "IPO date").await?;

    let fundamentals = Fundamentals {
        description,
        last_earnings_date: parse_tv_date(&earnings_date_str)?,
        ipo_date: parse_tv_date(&ipo_date_str)?,
    };

    println!("IPO date:      {}", fundamentals.ipo_date);
    println!("Earnings date: {}", fundamentals.last_earnings_date);
    println!("Description:   {}", fundamentals.description);

    Ok(fundamentals)
}

fn parse_tv_date(s: &str) -> anyhow::Result<NaiveDate> {
    // TradingView format: "Mar 5, 2026" or "Dec 4, 1985"
    // Zero-pad the day so chrono's %d can parse it
    let (month, rest) = s
        .split_once(' ')
        .ok_or_else(|| anyhow::anyhow!("invalid date: {s}"))?;
    let (day, year) = rest
        .split_once(", ")
        .ok_or_else(|| anyhow::anyhow!("invalid date: {s}"))?;
    let normalized = format!("{month} {day:0>2}, {year}");
    NaiveDate::parse_from_str(&normalized, "%b %d, %Y")
        .map_err(|e| anyhow::anyhow!("failed to parse date '{s}': {e}"))
}

async fn extract_description(page: &Page) -> anyhow::Result<String> {
    let js = r#"
        Array.from(document.querySelectorAll('script[type="application/ld+json"]'))
            .map(s => { try { return JSON.parse(s.textContent); } catch(e) { return null; } })
            .filter(d => d && d['@type'] === 'Corporation')
            .map(d => d.description)[0] || ''
    "#;
    Ok(page.evaluate(js).await?.into_value::<String>()?)
}

async fn extract_labeled_value(page: &Page, label: &str) -> anyhow::Result<String> {
    let js = format!(
        r#"
        (function() {{
            for (const el of document.querySelectorAll('[class*="label"]')) {{
                if (el.textContent.trim() === '{label}') {{
                    const block = el.closest('[class*="block"]');
                    if (block) {{
                        const val = block.querySelector('[class*="value"]');
                        if (val) return val.textContent.trim();
                    }}
                }}
            }}
            return '';
        }})()
        "#
    );

    for _ in 0..10 {
        let result = page.evaluate(js.as_str()).await?.into_value::<String>()?;
        if !result.is_empty() {
            return Ok(result);
        }
        page.sleep().await;
    }

    anyhow::bail!("could not find label '{label}' on page")
}

async fn launch_browser() -> anyhow::Result<Browser> {
    let cfg = config::config();
    let browser = ChromeDriverConfig::new(&cfg.chrome_path)
        .user_data_dir(&cfg.user_data_dir)
        .args(cfg.chrome_args.iter().map(|s| s.as_str()))
        .launch_if_needed(cfg.launch_if_needed)
        .connect()
        .await?;

    Ok(browser)
}
