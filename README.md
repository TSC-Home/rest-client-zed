# REST Client for Zed

A lightweight HTTP/REST client extension for [Zed](https://zed.dev) that lets you send requests directly from `.http` and `.rest` files.

![Zed REST Client](https://img.shields.io/badge/Zed-Extension-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- **Send HTTP requests** from `.http` / `.rest` files with a single click
- **Environment variables** via `{{VARIABLE}}` syntax with `.env` file support
- **Syntax highlighting** for HTTP methods, headers, URLs, and JSON bodies
- **Hover inspection** — hover over `{{VAR}}` to see its resolved value
- **Diagnostics** — warnings for unresolved variables and invalid URLs
- **Document symbols** — jump to named requests via `# @name`
- **Workspace search** — find requests across all `.http` files in your project
- **Interactive picker** — select and run named requests from the terminal
- **Pretty-printed responses** — formatted JSON, status codes, timing, and size

## Installation

Search for **"REST Client"** in the Zed extensions panel (`zed: extensions` in the command palette) and click **Install**.

### Manual Installation

```bash
git clone https://github.com/tsc-home/zed-rest
cd zed-rest
cargo install --path lsp
bash scripts/build.sh
bash scripts/install-dev.sh
```

## Usage

### Writing Requests

Create a `.http` or `.rest` file:

```http
# @name get-users
GET https://api.example.com/users
Accept: application/json

###

# @name create-user
POST https://api.example.com/users
Content-Type: application/json

{
  "name": "Jane Doe",
  "email": "jane@example.com"
}
```

- Separate multiple requests with `###`
- Name requests with `# @name my-request` for easy navigation

### Running Requests

Click the **Run** button that appears above each request method, or use the **"HTTP: Run Named Request"** task from the command palette to pick from all named requests.

### Keybindings

You can bind the tasks to keyboard shortcuts for faster access. Add this to your Zed `keymap.json`:

```json
[
  {
    "context": "Workspace",
    "bindings": {
      "ctrl-shift-r": ["task::Spawn", { "task_name": "Send HTTP Request" }],
      "ctrl-shift-p": ["task::Spawn", { "task_name": "HTTP: Run Named Request" }]
    }
  }
]
```

| Shortcut | Action |
|---|---|
| `ctrl-shift-r` | Send the request under the cursor |
| `ctrl-shift-p` | Open the interactive request picker |

> Customize these shortcuts to your preference in **Zed > Settings > Key Bindings** or by editing `~/.config/zed/keymap.json`.

### Environment Variables

Create a `.env` file in your project root or next to your `.http` file:

```env
HOST=api.example.com
TOKEN=my-secret-token
```

Reference variables in your requests:

```http
GET https://{{HOST}}/users
Authorization: Bearer {{TOKEN}}
```

The extension loads `.env`, `.env.local`, and `.env.development` files. When both exist, variables defined next to the `.http` file take precedence over those in the project root.

### CLI Usage

The binary can also be used standalone from the terminal:

```bash
# Execute a specific request by file and line number
rest-cli --execute path/to/api.http 5

# Interactive picker for named requests
rest-cli --pick .
```

## Supported Methods

`GET` · `POST` · `PUT` · `DELETE` · `PATCH` · `HEAD` · `OPTIONS` · `TRACE` · `CONNECT`

## How It Works

The extension consists of two components:

1. **WASM Extension** — integrates with Zed's extension API to register the language and launch the LSP
2. **Native LSP Server** — provides hover, diagnostics, symbols, and request execution via [tower-lsp](https://github.com/ebkalderon/tower-lsp) and [reqwest](https://github.com/seanmonstar/reqwest)

## License

[MIT](LICENSE)
