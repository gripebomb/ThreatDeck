use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::time::Duration;

pub fn fetch_article_text(url: &str) -> Result<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent("ThreatDeck/0.1 article reader")
        .build();
    let body = agent
        .get(url)
        .call()
        .with_context(|| format!("fetching article URL: {}", url))?
        .into_string()
        .context("reading article response body")?;
    extract_readable_text(&body)
}

pub fn extract_readable_text(html: &str) -> Result<String> {
    let document = Html::parse_document(html);
    for selector in ["article", "main", "body"] {
        let selector = Selector::parse(selector).expect("static selector is valid");
        if let Some(node) = document.select(&selector).next() {
            let text = collect_articleish_text(&node);
            if !text.is_empty() {
                return Ok(text);
            }
        }
    }
    anyhow::bail!("article did not contain readable text")
}

fn collect_articleish_text(node: &scraper::ElementRef<'_>) -> String {
    let text_selector = Selector::parse("h1, h2, h3, h4, p, li, blockquote, pre")
        .expect("static selector is valid");
    let parts: Vec<String> = node
        .select(&text_selector)
        .map(|child| normalize_whitespace(&child.text().collect::<Vec<_>>().join(" ")))
        .filter(|part| !part.is_empty())
        .collect();

    if !parts.is_empty() {
        return parts.join("\n\n");
    }

    normalize_whitespace(&node.text().collect::<Vec<_>>().join(" "))
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_readable_text_prefers_article_body() {
        let html = r#"
            <html>
              <body>
                <nav>Navigation noise</nav>
                <article>
                  <h1>Important threat report</h1>
                  <p>First paragraph &amp; context.</p>
                  <p>Second paragraph with indicators.</p>
                </article>
              </body>
            </html>
        "#;

        let text = extract_readable_text(html).unwrap();

        assert!(text.contains("Important threat report"));
        assert!(text.contains("First paragraph & context."));
        assert!(text.contains("Second paragraph with indicators."));
        assert!(!text.contains("Navigation noise"));
    }
}
