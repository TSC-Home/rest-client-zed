use zed_extension_api::{self as zed, LanguageServerId, Result};

struct RestExtension;

impl zed::Extension for RestExtension {
    fn new() -> Self {
        RestExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree
            .which("rest-cli")
            .ok_or_else(|| "rest-cli not found in PATH. Install it with: cargo install --path lsp".to_string())?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: vec![],
        })
    }
}

zed::register_extension!(RestExtension);
