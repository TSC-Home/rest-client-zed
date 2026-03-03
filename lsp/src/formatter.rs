use crate::executor::HttpResponse;

/// Format an HTTP response for display in the editor.
/// Output looks like a readable HTTP response with clear sections.
pub fn format_response(response: &HttpResponse) -> String {
    let mut output = String::new();

    // Status line with separator
    output.push_str(&format!(
        "# HTTP {} {} | {}ms | {}\n",
        response.status,
        response.status_text,
        response.elapsed_ms,
        format_size(response.size_bytes)
    ));
    output.push_str("#\n");
    output.push_str(&format!(
        "# {}\n",
        status_emoji(response.status)
    ));
    output.push('\n');

    // Headers
    for (key, value) in &response.headers {
        output.push_str(&format!("{key}: {value}\n"));
    }
    output.push('\n');

    // Body (pretty-print JSON if possible)
    let body = try_pretty_json(&response.body).unwrap_or_else(|| response.body.clone());
    output.push_str(&body);
    if !body.ends_with('\n') {
        output.push('\n');
    }

    output
}

/// Format response for CLI terminal output with ANSI colors.
pub fn format_response_cli(response: &HttpResponse) -> String {
    let mut output = String::new();

    // Colored status line
    let status_color = if response.status < 300 {
        "\x1b[1;32m" // green
    } else if response.status < 400 {
        "\x1b[1;33m" // yellow
    } else {
        "\x1b[1;31m" // red
    };

    output.push_str(&format!(
        "{status_color}HTTP {} {}\x1b[0m  \x1b[2m{}ms | {}\x1b[0m\n\n",
        response.status,
        response.status_text,
        response.elapsed_ms,
        format_size(response.size_bytes)
    ));

    // Dimmed headers
    for (key, value) in &response.headers {
        output.push_str(&format!("\x1b[2m{key}:\x1b[0m {value}\n"));
    }
    output.push('\n');

    // Body
    let body = try_pretty_json(&response.body).unwrap_or_else(|| response.body.clone());
    output.push_str(&body);
    if !body.ends_with('\n') {
        output.push('\n');
    }

    output
}

fn status_emoji(status: u16) -> &'static str {
    match status {
        200..=299 => "Success",
        300..=399 => "Redirect",
        400..=499 => "Client Error",
        500..=599 => "Server Error",
        _ => "",
    }
}

fn try_pretty_json(s: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(s).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
