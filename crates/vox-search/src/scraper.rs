use ::html2text::from_read;
use ::scraper::{Html, Selector};
use std::time::Duration;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct ScrapedDocument {
    pub url: String,
    pub title: String,
    pub markdown: String,
    pub text_density: f64,
}

pub async fn fetch_and_extract(url: &str, timeout_ms: u64) -> anyhow::Result<ScrapedDocument> {
    let client = vox_http_client::client_builder()
        .timeout(Duration::from_millis(timeout_ms))
        .user_agent("VoxResearchBot/1.0 (+https://vox.dev/research-bot)")
        .build()?;

    let resp = client.get(url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("Failed to fetch URL {}: {}", url, status));
    }

    let html_content = resp.text().await?;
    let document = Html::parse_document(&html_content);

    // 1. Extract Title
    let title_selector = Selector::parse("title").unwrap();
    let title = document
        .select(&title_selector)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_else(|| url.to_string());

    // 2. Prune boilerplate (nav, footer, script, style, aside, header)
    // Note: scraper doesn't make it easy to mutate the tree for removal.
    // We'll use a selection-based approach or just rely on the converted text quality.
    // However, html2text often converts everything.

    // A simplified "readability" attempt: target the <main> or <article> if they exist.
    let main_selector =
        Selector::parse("main, article, #content, .content, .post-content").unwrap();
    let target_html = if let Some(main_el) = document.select(&main_selector).next() {
        main_el.html()
    } else {
        // Fallback to body
        let body_selector = Selector::parse("body").unwrap();
        if let Some(body_el) = document.select(&body_selector).next() {
            body_el.html()
        } else {
            html_content.clone()
        }
    };

    // 3. Convert to Markdown
    let markdown = from_read(target_html.as_bytes(), 80);

    // 4. Text Density Heuristic
    let total_chars = html_content.len() as f64;
    let text_chars = markdown.len() as f64;
    let text_density = if total_chars > 0.0 {
        text_chars / total_chars
    } else {
        0.0
    };

    debug!(
        url = url,
        title = %title,
        density = text_density,
        "Scraped document"
    );

    Ok(ScrapedDocument {
        url: url.to_string(),
        title,
        markdown,
        text_density,
    })
}
