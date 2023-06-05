use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;

pub async fn fetch_body(url: &str, headers: &Option<HashMap<String, String>>) -> Result<String> {
    let mut request = Client::new().get(url);
    if headers.is_some() {
        for header in headers.as_ref().unwrap() {
            request = request.header(header.0, header.1);
        }
    }

    let response = request.send().await?;
    Ok(response.text().await?)
}
