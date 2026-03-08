use anyhow::Context;
use chrome_driver::{Browser, Page, Sleepable};
use chrono::Utc;
use model::Ticker;
use model::sentiment::{Forecast, NewsHeadline, StockSentiment};
use tracing::info;

pub struct SentimentScraper {
    browser: Option<Browser>,
    page: Option<Page>,
}

impl Drop for SentimentScraper {
    fn drop(&mut self) {
        if let Some(browser) = self.browser.take()
            && let Some(mut page) = self.page.take()
        {
            tokio::task::spawn(async move {
                page.close_me().await;
                drop(browser);
            });
        }
    }
}

impl SentimentScraper {
    pub async fn new() -> anyhow::Result<Self> {
        let browser = super::launch_browser().await?;
        let page = browser.new_page(super::TV_HOME).await?;
        page.wait_for_navigation().await?.sleep().await;
        Ok(Self {
            browser: Some(browser),
            page: Some(page),
        })
    }

    fn page(&self) -> &Page {
        self.page.as_ref().unwrap()
    }

    pub async fn scrape(&self, ticker: &Ticker) -> anyhow::Result<StockSentiment> {
        info!("Navigating to Overview page of {ticker}...");
        self.page()
            .goto(format!(
                "{}/symbols/{}-{}/",
                super::TV_HOME,
                ticker.exchange,
                ticker.ticker
            ))
            .await?
            .wait_for_navigation()
            .await?
            .sleep()
            .await;

        let (sector, industry, about) = self.scrape_overview().await.unwrap_or_else(|e| {
            tracing::warn!("Overview data unavailable for {ticker}: {e:#}");
            (String::new(), String::new(), String::new())
        });

        info!("Navigating to News/Analysts page of {ticker}...");
        self.page()
            .goto(format!(
                "{}/symbols/{}-{}/news/?section=recommendation",
                super::TV_HOME,
                ticker.exchange,
                ticker.ticker
            ))
            .await?
            .wait_for_navigation()
            .await?
            .sleep()
            .await;

        info!("Clicking Analysts news tab for {ticker}...");
        self.page()
            .evaluate(
                r#"const s = document.querySelector('[data-qa-id="symbol-page-news-section-content"]');
                   if (s) s.querySelector('[id="recommendation"]')?.click();"#,
            )
            .await?;
        self.page().sleep().await;

        let analyst_news = self.scrape_news().await.unwrap_or_else(|e| {
            tracing::warn!("Analyst news unavailable for {ticker}: {e:#}");
            vec![]
        });

        info!("Navigating to forecast page of {ticker}...");
        self.page()
            .goto(format!(
                "{}/symbols/{}-{}/forecast",
                super::TV_HOME,
                ticker.exchange,
                ticker.ticker
            ))
            .await?
            .wait_for_navigation()
            .await?
            .sleep()
            .await;

        let forecast = self.scrape_forecast().await.map_or_else(
            |e| {
                tracing::warn!("Forecast unavailable for {ticker}: {e:#}");
                None
            },
            Some,
        );

        Ok(StockSentiment {
            ticker: ticker.ticker.clone(),
            sector,
            industry,
            about,
            analyst_news,
            forecast,
            scraped_at: Utc::now(),
        })
    }

    async fn scrape_news(&self) -> anyhow::Result<Vec<NewsHeadline>> {
        let data = self.evaluate_news_js().await?;
        let arr = data.as_array().context("news JS did not return an array")?;
        let items = arr
            .iter()
            .filter_map(|item| {
                let headline = item["headline"].as_str()?.to_string();
                let event_time = item["event_time"].as_str()?;
                let published_at = chrono::DateTime::parse_from_rfc2822(event_time)
                    .ok()?
                    .with_timezone(&chrono::Utc);
                Some(NewsHeadline {
                    headline,
                    published_at,
                })
            })
            .collect();
        Ok(items)
    }

    async fn evaluate_news_js(&self) -> anyhow::Result<serde_json::Value> {
        const JS: &str = r#"(function() {
            const results = [];
            for (const card of document.querySelectorAll('[data-qa-id="news-headline-card"]')) {
                // Recent items use <relative-time event-time="RFC2822">,
                // older items use <time datetime="RFC2822"> — handle both.
                const rtEl   = card.querySelector('relative-time[event-time]');
                const timeEl = rtEl || card.querySelector('time[datetime]');
                const titleEl = card.querySelector('[data-qa-id="news-headline-title"]');
                if (!timeEl || !titleEl) continue;
                const event_time = timeEl.getAttribute('event-time')
                    || timeEl.getAttribute('datetime');
                // prefer data-overflow-tooltip-text — it holds the full untruncated title
                const headline = titleEl.getAttribute('data-overflow-tooltip-text')
                    || titleEl.textContent.trim();
                if (event_time && headline) results.push({ headline, event_time });
            }
            return results;
        })()"#;

        let result = self.page().evaluate(JS).await?;
        let raw = result.value().cloned();
        result.into_value().with_context(|| {
            format!("News JS extractor returned a non-deserializable value; raw: {raw:?}")
        })
    }

    async fn scrape_overview(&self) -> anyhow::Result<(String, String, String)> {
        let data = self.evaluate_overview_js().await?;
        let sector = data["sector"].as_str().unwrap_or("").to_string();
        let industry = data["industry"].as_str().unwrap_or("").to_string();
        let about = data["about"].as_str().unwrap_or("").to_string();
        Ok((sector, industry, about))
    }

    async fn evaluate_overview_js(&self) -> anyhow::Result<serde_json::Value> {
        const JS: &str = r#"(function() {
            // Sector and Industry live inside the "About <Company>" info block.
            // Each row is a block-QCJM7wcY element with a label-QCJM7wcY and
            // a value-QCJM7wcY child.
            const info = document.querySelector('[data-qa-id="company-info-id-content"]');
            const kv = {};
            if (info) {
                for (const block of info.querySelectorAll('[class*="block-QCJM7wcY"]')) {
                    const label = block.querySelector('[class*="label-QCJM7wcY"]');
                    const value = block.querySelector('[class*="value-QCJM7wcY"]');
                    if (label && value) {
                        kv[label.textContent.trim()] = value.textContent.trim();
                    }
                }
            }

            // Description text is in the truncated block-text container.
            // The actual prose is in the innermost <span>.
            const aboutEl = document.querySelector('[class*="content-H16icEW0"] span > span');
            const about = aboutEl ? aboutEl.textContent.trim() : null;

            return {
                sector:   kv['Sector']   || null,
                industry: kv['Industry'] || null,
                about,
            };
        })()"#;

        let result = self.page().evaluate(JS).await?;
        let raw = result.value().cloned();
        result.into_value().with_context(|| {
            format!("Overview JS extractor returned a non-deserializable value; raw: {raw:?}")
        })
    }

    async fn scrape_forecast(&self) -> anyhow::Result<Forecast> {
        let data = self.evaluate_forecast_js().await?;
        if data.is_null() {
            anyhow::bail!("forecast page not available (no forecast content found)");
        }
        parse_forecast(data)
    }

    async fn evaluate_forecast_js(&self) -> anyhow::Result<serde_json::Value> {
        const JS: &str = r#"(function() {
            // Return null if this isn't a forecast page (404 or no analyst coverage).
            // forecastPage- is a stable class present on all valid forecast pages.
            if (!document.querySelector('[class*="forecastPage-"]')) return null;

            // --- Price target ---
            // Anchor everything to priceLargeContainer — the page also has a stock-header
            // ticker widget that shows the current stock price, which would be picked up
            // first by a global querySelector('[data-qa-id="value"]').
            // Querying DOWN from the container avoids any ambiguity.
            const ptContainer = document.querySelector('[class*="priceLargeContainer-"]');
            const avgEl = ptContainer
                ? ptContainer.querySelector('[data-qa-id="value"]') : null;
            const avg = avgEl ? parseFloat(avgEl.textContent.trim()) : null;

            let upside_abs = null, upside_pct = null;
            if (ptContainer) {
                const changeEls = Array.from(
                    ptContainer.querySelectorAll('[class*="change-"]')
                );
                if (changeEls.length >= 2) {
                    const a = parseFloat(changeEls[0].textContent.trim());
                    const p = parseFloat(changeEls[1].textContent.trim());
                    if (!isNaN(a)) upside_abs = a;
                    if (!isNaN(p)) upside_pct = p;
                }
            }

            // Current stock price = avg target − absolute upside
            const current = (avg != null && upside_abs != null) ? avg - upside_abs : null;

            // "The N analysts … max estimate of X … min estimate of Y"
            let pt_count = null, pt_max = null, pt_min = null;
            for (const el of document.querySelectorAll('[class*="sectionSubtitle-"]')) {
                const t = el.textContent;
                if (!t.includes('max estimate')) continue;
                const cm = t.match(/The\s+(\d+)\s+analyst/);
                const mx = t.match(/max estimate of\s+([\d.]+)/);
                const mn = t.match(/min estimate of\s+([\d.]+)/);
                if (cm) pt_count = parseInt(cm[1]);
                if (mx) pt_max = parseFloat(mx[1]);
                if (mn) pt_min = parseFloat(mn[1]);
                break;
            }

            // --- Analyst rating ---
            let rating_total = null;
            for (const el of document.querySelectorAll('[class*="sectionSubtitle-"]')) {
                const t = el.textContent;
                if (!t.includes('analysts giving stock ratings')) continue;
                const m = t.match(/(\d+)\s+analysts/);
                if (m) rating_total = parseInt(m[1]);
                break;
            }

            // Rating bar counts: title-NCbKhExY / value-NCbKhExY pairs
            const ratingMap = {};
            const wrap = document.querySelector('[class*="wrap-NCbKhExY"]');
            if (wrap) {
                const titles = wrap.querySelectorAll('[class*="title-NCbKhExY"]');
                const vals   = wrap.querySelectorAll('[class*="value-NCbKhExY"]');
                for (let i = 0; i < titles.length && i < vals.length; i++) {
                    ratingMap[titles[i].textContent.trim()] =
                        parseInt(vals[i].textContent.trim()) || 0;
                }
            }

            // Consensus: compute from counts using TradingView's weighted 1–5 scale.
            // CSS class–based detection is fragile (hash suffix varies by deploy and the
            // selector can match unrelated elements).  Weighted math produces the same
            // label TradingView shows:  ≥4.5 → Strong Buy, ≥3.5 → Buy, ≥2.5 → Neutral,
            // ≥1.5 → Sell, else → Strong Sell.
            const _sb = ratingMap['Strong buy']  ?? 0;
            const _b  = ratingMap['Buy']         ?? 0;
            const _h  = ratingMap['Hold']        ?? 0;
            const _s  = ratingMap['Sell']        ?? 0;
            const _ss = ratingMap['Strong sell'] ?? 0;
            const _tot = _sb + _b + _h + _s + _ss;
            let consensus = null;
            if (_tot > 0) {
                const score = (_sb * 5 + _b * 4 + _h * 3 + _s * 2 + _ss * 1) / _tot;
                if      (score >= 4.5) consensus = 'Strong Buy';
                else if (score >= 3.5) consensus = 'Buy';
                else if (score >= 2.5) consensus = 'Neutral';
                else if (score >= 1.5) consensus = 'Sell';
                else                   consensus = 'Strong Sell';
            }

            return {
                price_target_average:           avg,
                price_target_upside_abs:        upside_abs,
                price_target_average_upside_pct: upside_pct,
                price_current:           current,
                price_target_max:               pt_max,
                price_target_min:               pt_min,
                price_target_analyst_count:     pt_count,
                rating_total_analysts:          rating_total,
                rating_strong_buy:  ratingMap['Strong buy']  ?? null,
                rating_buy:         ratingMap['Buy']         ?? null,
                rating_hold:        ratingMap['Hold']        ?? null,
                rating_sell:        ratingMap['Sell']        ?? null,
                rating_strong_sell: ratingMap['Strong sell'] ?? null,
                rating_consensus:   consensus,
            };
        })()"#;

        let result = self.page().evaluate(JS).await?;
        let raw = result.value().cloned();
        result.into_value().with_context(|| {
            format!("Forecast JS extractor returned a non-deserializable value; raw: {raw:?}")
        })
    }
}

fn f64_opt(v: &serde_json::Value, key: &str) -> Option<f64> {
    v[key].as_f64()
}

fn u32_opt(v: &serde_json::Value, key: &str) -> Option<u32> {
    v[key].as_u64().map(|n| n as u32)
}

fn parse_forecast(data: serde_json::Value) -> anyhow::Result<Forecast> {
    let current = f64_opt(&data, "price_current");
    let average = f64_opt(&data, "price_target_average");
    let max = f64_opt(&data, "price_target_max");
    let min = f64_opt(&data, "price_target_min");

    let max_upside_pct = current.zip(max).map(|(c, m)| (m - c) / c * 100.0);
    let min_downside_pct = current.zip(min).map(|(c, mn)| (mn - c) / c * 100.0);

    Ok(Forecast {
        price_current: current,
        price_target_average: average,
        price_target_average_upside_pct: f64_opt(&data, "price_target_average_upside_pct"),
        price_target_max: max,
        price_target_max_upside_pct: max_upside_pct,
        price_target_min: min,
        price_target_min_downside_pct: min_downside_pct,
        price_target_analyst_count: u32_opt(&data, "price_target_analyst_count"),
        rating_strong_buy: u32_opt(&data, "rating_strong_buy"),
        rating_buy: u32_opt(&data, "rating_buy"),
        rating_hold: u32_opt(&data, "rating_hold"),
        rating_sell: u32_opt(&data, "rating_sell"),
        rating_strong_sell: u32_opt(&data, "rating_strong_sell"),
        rating_total_analysts: u32_opt(&data, "rating_total_analysts"),
        rating_consensus: data["rating_consensus"].as_str().map(str::to_string),
    })
}
