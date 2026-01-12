//! HTTP utilities for URL import

use anyhow::Result;
use std::time::Duration;
use url::Url;

/// Fetch content from a URL
pub fn fetch_url(url_str: &str) -> Result<String> {
    let url = Url::parse(url_str)?;

    // Validate scheme
    if url.scheme() != "https" && url.scheme() != "http" {
        anyhow::bail!("Only HTTP/HTTPS URLs are supported");
    }

    // Create a client with timeout
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Perform the request
    let response = client.get(url_str).send()?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP request failed with status: {}", response.status());
    }

    let content = response.text()?;
    Ok(content)
}

/// Check if a string is a valid URL
pub fn is_url(s: &str) -> bool {
    if let Ok(url) = Url::parse(s) {
        url.scheme() == "http" || url.scheme() == "https"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_url() {
        assert!(is_url("https://example.com/file.sh"));
        assert!(is_url("http://example.com/file.sh"));
        assert!(!is_url("/home/user/file.sh"));
        assert!(!is_url("file.sh"));
    }
}
