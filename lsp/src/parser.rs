use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Line number (0-indexed) of the method line
    pub line: u32,
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Request URL (with variables resolved)
    pub url: String,
    /// Raw URL (before variable resolution)
    pub raw_url: String,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body (if any)
    pub body: Option<String>,
    /// Request name/label (from `# @name` comment)
    pub name: Option<String>,
}

static VARIABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{(\w+)\}\}").expect("invalid regex"));

static METHOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|TRACE|CONNECT)\s+(.+?)(?:\s+HTTP/[\d.]+)?\s*$")
        .expect("invalid regex")
});

static HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\w-]+)\s*:\s*(.+)\s*$").expect("invalid regex"));

static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#\s*@name\s+(.+)\s*$").expect("invalid regex"));

/// Parse a `.http` file into a list of requests.
pub fn parse_http_file(content: &str) -> Vec<HttpRequest> {
    let mut requests = Vec::new();
    let blocks = split_request_blocks(content);

    for block in blocks {
        if let Some(req) = parse_request_block(&block.lines, block.start_line) {
            requests.push(req);
        }
    }

    requests
}

struct RequestBlock {
    lines: Vec<String>,
    start_line: u32,
}

fn split_request_blocks(content: &str) -> Vec<RequestBlock> {
    let mut blocks = Vec::new();
    let mut current_lines = Vec::new();
    let mut current_start = 0u32;
    let mut found_content = false;

    for (i, line) in content.lines().enumerate() {
        if line.starts_with("###") {
            if found_content {
                blocks.push(RequestBlock {
                    lines: std::mem::take(&mut current_lines),
                    start_line: current_start,
                });
            }
            current_start = (i + 1) as u32;
            found_content = false;
            continue;
        }

        if !found_content && !line.trim().is_empty() && !line.starts_with('#') {
            current_start = i as u32;
        }

        if !line.trim().is_empty() {
            found_content = true;
        }

        current_lines.push(line.to_string());
    }

    if found_content {
        blocks.push(RequestBlock {
            lines: current_lines,
            start_line: current_start,
        });
    }

    blocks
}

fn parse_request_block(lines: &[String], start_line: u32) -> Option<HttpRequest> {
    let mut name = None;
    let mut method_line_idx = None;
    let mut method = String::new();
    let mut raw_url = String::new();

    // Find method line, skipping comments and blank lines
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // Check for @name annotation
        if let Some(caps) = NAME_RE.captures(trimmed) {
            name = Some(caps[1].to_string());
            continue;
        }

        if trimmed.starts_with('#') {
            continue;
        }

        // Try to match method line
        if let Some(caps) = METHOD_RE.captures(trimmed) {
            method = caps[1].to_string();
            raw_url = caps[2].to_string();
            method_line_idx = Some(i);
            break;
        }
    }

    let method_line_idx = method_line_idx?;
    let actual_line = start_line + method_line_idx as u32;

    // Parse headers (lines after method until blank line)
    let mut headers = HashMap::new();
    let mut body_start = None;

    for (i, line) in lines.iter().enumerate().skip(method_line_idx + 1) {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            body_start = Some(i + 1);
            break;
        }

        if let Some(caps) = HEADER_RE.captures(trimmed) {
            headers.insert(caps[1].to_string(), caps[2].to_string());
        }
    }

    // Parse body (everything after blank line)
    let body = body_start.and_then(|start| {
        let body_lines: Vec<&str> = lines[start..]
            .iter()
            .map(|s| s.as_str())
            .collect();
        let body_text = body_lines.join("\n").trim().to_string();
        if body_text.is_empty() {
            None
        } else {
            Some(body_text)
        }
    });

    Some(HttpRequest {
        line: actual_line,
        method,
        url: raw_url.clone(),
        raw_url,
        headers,
        body,
        name,
    })
}

/// Resolve `{{VAR}}` placeholders in a request using environment variables.
pub fn resolve_variables(request: &mut HttpRequest, env: &HashMap<String, String>) {
    let resolve = |s: &str| -> String {
        VARIABLE_RE
            .replace_all(s, |caps: &regex::Captures| {
                let var_name = &caps[1];
                env.get(var_name)
                    .cloned()
                    .unwrap_or_else(|| format!("{{{{{var_name}}}}}"))
            })
            .to_string()
    };

    request.url = resolve(&request.raw_url);

    let resolved_headers: HashMap<String, String> = request
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), resolve(v)))
        .collect();
    request.headers = resolved_headers;

    if let Some(body) = &request.body {
        request.body = Some(resolve(body));
    }
}

/// Load environment variables with workspace-root fallback.
///
/// Loads from `root` first, then overlays values from `file_dir` (if different).
/// This means variables defined next to the `.http` file take precedence over
/// those in the workspace root.
pub fn load_env_merged(root: &Path, file_dir: &Path) -> HashMap<String, String> {
    let mut env = load_env_files(root);
    if file_dir != root {
        env.extend(load_env_files(file_dir));
    }
    env
}

/// Find the workspace root by walking up from `start` looking for common root markers.
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?
    } else {
        start
    };
    loop {
        for marker in &[".git", ".env", "Cargo.toml", "package.json", ".projectroot"] {
            if dir.join(marker).exists() {
                return Some(dir.to_path_buf());
            }
        }
        dir = dir.parent()?;
    }
}

/// Load environment variables from `.env` files in the given directory.
pub fn load_env_files(workspace_root: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();

    let env_files = [".env", ".env.local", ".env.development"];

    for filename in &env_files {
        let path = workspace_root.join(filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = trimmed.split_once('=') {
                    let key = key.trim().to_string();
                    let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
                    env.insert(key, value);
                }
            }
        }
    }

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_get() {
        let content = "GET https://example.com/api/users\n";
        let requests = parse_http_file(content);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].url, "https://example.com/api/users");
    }

    #[test]
    fn test_parse_post_with_body() {
        let content = r#"POST https://example.com/api/users
Content-Type: application/json

{
  "name": "test"
}"#;
        let requests = parse_http_file(content);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(
            requests[0].headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert!(requests[0].body.is_some());
    }

    #[test]
    fn test_parse_multiple_requests() {
        let content = r#"GET https://example.com/api/users

###

POST https://example.com/api/users
Content-Type: application/json

{"name": "test"}
"#;
        let requests = parse_http_file(content);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[1].method, "POST");
    }

    #[test]
    fn test_variable_resolution() {
        let mut req = HttpRequest {
            line: 0,
            method: "GET".into(),
            url: "https://{{HOST}}/api".into(),
            raw_url: "https://{{HOST}}/api".into(),
            headers: HashMap::from([("Authorization".into(), "Bearer {{TOKEN}}".into())]),
            body: None,
            name: None,
        };

        let env = HashMap::from([
            ("HOST".into(), "example.com".into()),
            ("TOKEN".into(), "abc123".into()),
        ]);

        resolve_variables(&mut req, &env);
        assert_eq!(req.url, "https://example.com/api");
        assert_eq!(req.headers.get("Authorization").unwrap(), "Bearer abc123");
    }
}
