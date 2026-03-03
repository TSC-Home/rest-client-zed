mod executor;
mod formatter;
mod parser;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use parser::HttpRequest;

fn log_to_file(msg: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("zed-rest-lsp.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[rest-lsp] {msg}");
    }
}

#[derive(Debug)]
struct Document {
    content: String,
    requests: Vec<HttpRequest>,
}

struct RestLsp {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, Document>>>,
    workspace_root: Arc<RwLock<Option<PathBuf>>>,
}

impl RestLsp {
    fn new(client: Client) -> Self {
        RestLsp {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            workspace_root: Arc::new(RwLock::new(None)),
        }
    }

    async fn update_document(&self, uri: &Url, content: String) {
        let requests = parser::parse_http_file(&content);
        let doc = Document {
            content,
            requests,
        };

        let mut docs = self.documents.write().await;
        docs.insert(uri.clone(), doc);
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri) else { return };

        let mut diagnostics = Vec::new();

        for req in &doc.requests {
            // Warn on URLs that still have unresolved variables
            if req.url.contains("{{") {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position {
                            line: req.line,
                            character: 0,
                        },
                        end: Position {
                            line: req.line,
                            character: 1000,
                        },
                    },
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("rest-lsp".into()),
                    message: "URL contains unresolved variables".into(),
                    ..Default::default()
                });
            }

            // Validate URL format
            if url::Url::parse(&req.url).is_err() && !req.url.contains("{{") {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position {
                            line: req.line,
                            character: 0,
                        },
                        end: Position {
                            line: req.line,
                            character: 1000,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("rest-lsp".into()),
                    message: "Invalid URL format".into(),
                    ..Default::default()
                });
            }
        }

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

}

#[tower_lsp::async_trait]
impl LanguageServer for RestLsp {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        log_to_file("initialize called");
        log_to_file(&format!("client codeLens cap: {:?}", params.capabilities.text_document.as_ref().and_then(|td| td.code_lens.as_ref())));

        // Store workspace root
        if let Some(folders) = params.workspace_folders {
            if let Some(folder) = folders.first() {
                if let Ok(path) = folder.uri.to_file_path() {
                    let mut root = self.workspace_root.write().await;
                    *root = Some(path);
                }
            }
        } else if let Some(uri) = params.root_uri {
            if let Ok(path) = uri.to_file_path() {
                let mut root = self.workspace_root.write().await;
                *root = Some(path);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        log_to_file("initialized notification received");
        self.client
            .log_message(MessageType::INFO, "REST LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        log_to_file(&format!("did_open: {}", params.text_document.uri));
        let uri = params.text_document.uri;
        self.update_document(&uri, params.text_document.text).await;
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.update_document(&uri, change.text).await;
            self.publish_diagnostics(&uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.documents.write().await;
        docs.remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri) else {
            return Ok(None);
        };

        // Check if cursor is on a {{variable}}
        let lines: Vec<&str> = doc.content.lines().collect();
        if let Some(line_text) = lines.get(position.line as usize) {
            let col = position.character as usize;

            // Find variable at cursor position
            for cap in regex::Regex::new(r"\{\{(\w+)\}\}")
                .unwrap()
                .captures_iter(line_text)
            {
                let m = cap.get(0).unwrap();
                if col >= m.start() && col <= m.end() {
                    let var_name = &cap[1];
                    let workspace_root = self.workspace_root.read().await;
                    let env = workspace_root
                        .as_ref()
                        .map(|r| parser::load_env_files(r))
                        .unwrap_or_default();

                    let value = env
                        .get(var_name)
                        .map(|v| format!("`{var_name}` = `{v}`"))
                        .unwrap_or_else(|| format!("`{var_name}` — **not defined**"));

                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value,
                        }),
                        range: Some(Range {
                            start: Position {
                                line: position.line,
                                character: m.start() as u32,
                            },
                            end: Position {
                                line: position.line,
                                character: m.end() as u32,
                            },
                        }),
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri) else {
            return Ok(None);
        };

        #[allow(deprecated)]
        let symbols: Vec<SymbolInformation> = doc
            .requests
            .iter()
            .filter(|r| r.name.is_some())
            .map(|req| SymbolInformation {
                name: format!("{} {}", req.method, req.name.as_deref().unwrap_or(&req.url)),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: Range {
                        start: Position { line: req.line, character: 0 },
                        end: Position { line: req.line, character: 0 },
                    },
                },
                container_name: None,
            })
            .collect();

        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let workspace_root = self.workspace_root.read().await;

        let Some(root) = workspace_root.as_ref() else {
            return Ok(None);
        };

        let mut symbols = Vec::new();
        let http_files = find_http_files(root);

        for file_path in http_files {
            let Ok(content) = std::fs::read_to_string(&file_path) else {
                continue;
            };
            let Ok(file_uri) = Url::from_file_path(&file_path) else {
                continue;
            };

            let requests = parser::parse_http_file(&content);

            #[allow(deprecated)]
            for req in &requests {
                let Some(name) = &req.name else { continue };
                let label = format!("{} {}", req.method, name);

                if !query.is_empty() && !label.to_lowercase().contains(&query) {
                    continue;
                }

                symbols.push(SymbolInformation {
                    name: label,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position { line: req.line, character: 0 },
                            end: Position { line: req.line, character: 0 },
                        },
                    },
                    container_name: Some(
                        file_path
                            .strip_prefix(root)
                            .unwrap_or(&file_path)
                            .to_string_lossy()
                            .to_string(),
                    ),
                });
            }
        }

        Ok(Some(symbols))
    }

}

fn find_http_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_http_files(root, &mut files, 0);
    files
}

fn collect_http_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>, depth: u32) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                collect_http_files(&path, files, depth + 1);
            }
        } else if let Some(ext) = path.extension() {
            if ext == "http" || ext == "rest" {
                files.push(path);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // CLI mode: --execute <file> <line>
    if args.len() >= 4 && args[1] == "--execute" {
        let file_path = &args[2];
        let line: u32 = args[3].parse().unwrap_or(1);
        run_request(file_path, line).await;
        return;
    }

    // Interactive picker: --pick [dir]
    if args.len() >= 2 && args[1] == "--pick" {
        let dir = if args.len() >= 3 { &args[2] } else { "." };
        pick_and_run(dir).await;
        return;
    }

    // LSP mode (default)
    let log_path = std::env::temp_dir().join("zed-rest-lsp.log");
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::File::create(&log_path) {
            let _ = writeln!(f, "[rest-lsp] starting at {:?}", std::time::SystemTime::now());
        }
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(RestLsp::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

async fn run_request(file_path: &str, line: u32) {
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {file_path}: {e}");
            std::process::exit(1);
        }
    };

    let requests = parser::parse_http_file(&content);

    // Find the request at or before the given line (1-indexed from Zed)
    let line_0 = if line > 0 { line - 1 } else { 0 };
    let request = requests
        .iter()
        .filter(|r| r.line <= line_0)
        .last()
        .or_else(|| requests.first());

    let Some(request) = request else {
        eprintln!("No HTTP request found in {file_path}");
        std::process::exit(1);
    };

    let mut request = request.clone();

    // Resolve variables: load from workspace root, overlay with file's directory
    let file_dir = std::path::Path::new(file_path)
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let root = parser::find_workspace_root(&file_dir)
        .unwrap_or_else(|| file_dir.clone());
    let env = parser::load_env_merged(&root, &file_dir);
    parser::resolve_variables(&mut request, &env);

    let label = request.name.as_deref().unwrap_or(&request.url);
    println!("\x1b[1;36m{} {}\x1b[0m", request.method, label);
    println!();

    execute_and_print(&request).await;
}

async fn pick_and_run(dir: &str) {
    let root = std::path::Path::new(dir);
    let http_files = find_http_files(root);

    if http_files.is_empty() {
        eprintln!("No .http/.rest files found in {dir}");
        std::process::exit(1);
    }

    // Collect all named requests
    struct NamedRequest {
        file_display: String,
        file_dir: std::path::PathBuf,
        request: parser::HttpRequest,
    }

    let mut named: Vec<NamedRequest> = Vec::new();

    for file_path in &http_files {
        let Ok(content) = std::fs::read_to_string(file_path) else {
            continue;
        };
        let requests = parser::parse_http_file(&content);
        let relative = file_path
            .strip_prefix(root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        let file_dir = file_path.parent().unwrap_or(root).to_path_buf();

        for req in requests {
            if req.name.is_some() {
                named.push(NamedRequest {
                    file_display: relative.clone(),
                    file_dir: file_dir.clone(),
                    request: req,
                });
            }
        }
    }

    if named.is_empty() {
        eprintln!("No named requests found (add \x1b[1m# @name my-request\x1b[0m above a request)");
        std::process::exit(1);
    }

    // Display the list
    println!("\x1b[1;35mHTTP Requests\x1b[0m");
    println!();
    for (i, entry) in named.iter().enumerate() {
        let name = entry.request.name.as_deref().unwrap();
        let method = &entry.request.method;
        let method_color = match method.as_str() {
            "GET" => "\x1b[1;32m",
            "POST" => "\x1b[1;33m",
            "PUT" => "\x1b[1;34m",
            "DELETE" => "\x1b[1;31m",
            "PATCH" => "\x1b[1;36m",
            _ => "\x1b[1;37m",
        };
        println!(
            "  \x1b[1;37m{:>2}\x1b[0m  {}{:<7}\x1b[0m \x1b[1m{}\x1b[0m  \x1b[2m({})\x1b[0m",
            i + 1,
            method_color,
            method,
            name,
            entry.file_display,
        );
    }
    println!();

    // Read choice
    eprint!("\x1b[1;35m#>\x1b[0m ");
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        std::process::exit(1);
    }

    let choice: usize = match input.trim().parse::<usize>() {
        Ok(n) if n >= 1 && n <= named.len() => n - 1,
        _ => {
            eprintln!("Invalid choice");
            std::process::exit(1);
        }
    };

    let entry = &named[choice];
    let mut request = entry.request.clone();

    // Resolve variables: load from workspace root, overlay with file's directory
    let env = parser::load_env_merged(root, &entry.file_dir);
    parser::resolve_variables(&mut request, &env);

    println!();
    let label = request.name.as_deref().unwrap_or(&request.url);
    println!("\x1b[1;36m{} {}\x1b[0m", request.method, label);
    println!();

    execute_and_print(&request).await;
}

async fn execute_and_print(request: &parser::HttpRequest) {
    let client = reqwest::Client::new();
    match executor::execute_request(&client, request).await {
        Ok(response) => {
            print!("{}", formatter::format_response_cli(&response));
        }
        Err(e) => {
            eprintln!("\x1b[1;31mRequest failed:\x1b[0m {e}");
            std::process::exit(1);
        }
    }
}
