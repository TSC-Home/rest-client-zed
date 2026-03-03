use crate::parser::HttpRequest;
use reqwest::Client;
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub elapsed_ms: u128,
    pub size_bytes: usize,
}

pub async fn execute_request(
    client: &Client,
    request: &HttpRequest,
) -> Result<HttpResponse, String> {
    let method: reqwest::Method = request
        .method
        .parse()
        .map_err(|e| format!("invalid method: {e}"))?;

    let mut builder = client.request(method, &request.url);

    for (key, value) in &request.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }

    let start = Instant::now();

    let response = builder.send().await.map_err(|e| format!("request failed: {e}"))?;

    let elapsed_ms = start.elapsed().as_millis();
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
        .collect();

    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| format!("failed to read body: {e}"))?;

    let size_bytes = body_bytes.len();
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    Ok(HttpResponse {
        status,
        status_text,
        headers,
        body,
        elapsed_ms,
        size_bytes,
    })
}
