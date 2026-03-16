//! gopls process lifecycle and low-level LSP communication.
//!
//! `GoplsClient` spawns a `gopls` child process, performs the LSP initialize
//! handshake, and exposes typed request methods. It shuts down cleanly on drop.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::protocol::{
    self, HoverResult, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, LspLocation, Position,
    ReferenceContext, ReferenceParams, TextDocumentIdentifier, TextDocumentPositionParams,
};

/// Default timeout for LSP requests.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Result type for gopls client operations.
type ClientResult<T> = Result<T, ClientError>;

/// Errors specific to the gopls client.
#[derive(Debug)]
pub enum ClientError {
    /// `gopls` binary not found on PATH.
    NotFound,
    /// Failed to spawn the `gopls` process.
    SpawnFailed(std::io::Error),
    /// The workspace root does not exist or is not a directory.
    InvalidWorkspace(PathBuf),
    /// LSP initialize handshake failed.
    InitializeFailed(String),
    /// A request timed out.
    Timeout { method: String, elapsed: Duration },
    /// IO error during communication.
    Io(std::io::Error),
    /// gopls returned a JSON-RPC error.
    RpcError { code: i64, message: String },
    /// The gopls process exited unexpectedly.
    ProcessExited,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NotFound => write!(f, "gopls not found on PATH"),
            ClientError::SpawnFailed(e) => write!(f, "failed to spawn gopls: {e}"),
            ClientError::InvalidWorkspace(p) => {
                write!(f, "invalid workspace: {}", p.display())
            }
            ClientError::InitializeFailed(msg) => {
                write!(f, "LSP initialize failed: {msg}")
            }
            ClientError::Timeout { method, elapsed } => {
                write!(f, "gopls request '{method}' timed out after {elapsed:?}")
            }
            ClientError::Io(e) => write!(f, "gopls IO error: {e}"),
            ClientError::RpcError { code, message } => {
                write!(f, "gopls error {code}: {message}")
            }
            ClientError::ProcessExited => write!(f, "gopls process exited unexpectedly"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        ClientError::Io(e)
    }
}

/// A running `gopls` instance with LSP communication.
pub struct GoplsClient {
    process: Child,
    next_id: AtomicU64,
    workspace_root: PathBuf,
    timeout: Duration,
}

impl GoplsClient {
    /// Start `gopls` and perform the LSP initialize handshake.
    ///
    /// Returns `Err(ClientError::NotFound)` if `gopls` is not on PATH.
    /// Returns `Err(ClientError::InvalidWorkspace)` if the path doesn't exist.
    pub fn start(workspace_root: &Path) -> ClientResult<Self> {
        Self::start_with_timeout(workspace_root, DEFAULT_TIMEOUT)
    }

    /// Start with a custom timeout for all requests.
    pub fn start_with_timeout(workspace_root: &Path, timeout: Duration) -> ClientResult<Self> {
        // Validate workspace.
        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|_| ClientError::InvalidWorkspace(workspace_root.to_path_buf()))?;

        if !workspace_root.is_dir() {
            return Err(ClientError::InvalidWorkspace(workspace_root));
        }

        // Check gopls exists.
        let gopls_path = which_gopls()?;

        // Spawn gopls serve (stdio mode).
        let process = Command::new(gopls_path)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ClientError::SpawnFailed)?;

        let mut client = Self {
            process,
            next_id: AtomicU64::new(1),
            workspace_root,
            timeout,
        };

        client.initialize()?;
        Ok(client)
    }

    /// Send the `initialize` request and `initialized` notification.
    fn initialize(&mut self) -> ClientResult<()> {
        let root_uri = protocol::path_to_uri(self.workspace_root.to_str().unwrap_or(""));

        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "definition": { "dynamicRegistration": false },
                    "references": { "dynamicRegistration": false },
                    "hover": { "dynamicRegistration": false }
                }
            },
            "workspaceFolders": [{
                "uri": root_uri,
                "name": "quality-service"
            }]
        });

        let resp = self.send_request("initialize", Some(params))?;
        if resp.error.is_some() {
            let err = resp.error.unwrap();
            return Err(ClientError::InitializeFailed(err.message));
        }

        // Send initialized notification.
        self.send_notification("initialized", None)?;
        Ok(())
    }

    // ── Public query methods ────────────────────────────────────────────

    /// Go to definition at a file position.
    pub fn definition(
        &mut self,
        file: &str,
        line: u32,
        character: u32,
    ) -> ClientResult<Vec<LspLocation>> {
        let params = serde_json::to_value(TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: protocol::path_to_uri(file),
            },
            position: Position { line, character },
        })
        .map_err(|e| ClientError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let resp = self.send_request("textDocument/definition", Some(params))?;
        self.check_rpc_error(&resp)?;

        match resp.result {
            Some(val) => {
                // gopls can return a single Location or an array.
                if val.is_array() {
                    Ok(serde_json::from_value(val).unwrap_or_default())
                } else if val.is_object() {
                    let loc: LspLocation =
                        serde_json::from_value(val).unwrap_or_else(|_| LspLocation {
                            uri: String::new(),
                            range: protocol::Range {
                                start: Position {
                                    line: 0,
                                    character: 0,
                                },
                                end: Position {
                                    line: 0,
                                    character: 0,
                                },
                            },
                        });
                    Ok(vec![loc])
                } else {
                    Ok(vec![])
                }
            }
            None => Ok(vec![]),
        }
    }

    /// Find all references to a symbol at a file position.
    pub fn references(
        &mut self,
        file: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> ClientResult<Vec<LspLocation>> {
        let params = serde_json::to_value(ReferenceParams {
            text_document: TextDocumentIdentifier {
                uri: protocol::path_to_uri(file),
            },
            position: Position { line, character },
            context: ReferenceContext {
                include_declaration,
            },
        })
        .map_err(|e| ClientError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let resp = self.send_request("textDocument/references", Some(params))?;
        self.check_rpc_error(&resp)?;

        match resp.result {
            Some(val) => Ok(serde_json::from_value(val).unwrap_or_default()),
            None => Ok(vec![]),
        }
    }

    /// Get hover information at a file position.
    pub fn hover(
        &mut self,
        file: &str,
        line: u32,
        character: u32,
    ) -> ClientResult<Option<HoverResult>> {
        let params = serde_json::to_value(TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: protocol::path_to_uri(file),
            },
            position: Position { line, character },
        })
        .map_err(|e| ClientError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let resp = self.send_request("textDocument/hover", Some(params))?;
        self.check_rpc_error(&resp)?;

        match resp.result {
            Some(Value::Null) | None => Ok(None),
            Some(val) => Ok(serde_json::from_value(val).ok()),
        }
    }

    /// Gracefully shut down the gopls process.
    pub fn shutdown(mut self) -> ClientResult<()> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> ClientResult<()> {
        // Send shutdown request.
        let resp = self.send_request("shutdown", None)?;
        self.check_rpc_error(&resp)?;

        // Send exit notification.
        self.send_notification("exit", None)?;

        // Wait briefly for process to exit.
        let _ = self.process.wait();
        Ok(())
    }

    // ── Low-level communication ─────────────────────────────────────────

    fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> ClientResult<JsonRpcResponse> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(id, method, params);
        let encoded = req.encode();

        let start = Instant::now();

        // Write request.
        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or(ClientError::ProcessExited)?;
        stdin.write_all(&encoded)?;
        stdin.flush()?;

        // Read response, skipping notifications/diagnostics from gopls.
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or(ClientError::ProcessExited)?;
        let mut reader = BufReader::new(stdout);

        loop {
            if start.elapsed() > self.timeout {
                return Err(ClientError::Timeout {
                    method: method.to_string(),
                    elapsed: start.elapsed(),
                });
            }

            let resp = protocol::read_message(&mut reader)?;

            // Skip notifications (no id).
            if resp.id.is_none() {
                continue;
            }

            // Match our request id.
            if resp.id == Some(id) {
                return Ok(resp);
            }
            // Otherwise keep reading (could be out-of-order responses).
        }
    }

    fn send_notification(&mut self, method: &str, params: Option<Value>) -> ClientResult<()> {
        let notif = JsonRpcNotification::new(method, params);
        let encoded = notif.encode();

        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or(ClientError::ProcessExited)?;
        stdin.write_all(&encoded)?;
        stdin.flush()?;
        Ok(())
    }

    fn check_rpc_error(&self, resp: &JsonRpcResponse) -> ClientResult<()> {
        if let Some(ref err) = resp.error {
            Err(ClientError::RpcError {
                code: err.code,
                message: err.message.clone(),
            })
        } else {
            Ok(())
        }
    }

    /// Get the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl Drop for GoplsClient {
    fn drop(&mut self) {
        // Best-effort shutdown — ignore errors.
        let _ = self.shutdown_inner();
        let _ = self.process.kill();
    }
}

/// Locate `gopls` on PATH.
fn which_gopls() -> ClientResult<PathBuf> {
    // Try common locations.
    let candidates = ["gopls"];
    for name in &candidates {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }
    }
    Err(ClientError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_error_display() {
        assert_eq!(ClientError::NotFound.to_string(), "gopls not found on PATH");
        assert_eq!(
            ClientError::InvalidWorkspace(PathBuf::from("/bad")).to_string(),
            "invalid workspace: /bad"
        );
        assert_eq!(
            ClientError::Timeout {
                method: "textDocument/hover".into(),
                elapsed: Duration::from_secs(5),
            }
            .to_string(),
            "gopls request 'textDocument/hover' timed out after 5s"
        );
        assert_eq!(
            ClientError::RpcError {
                code: -32600,
                message: "invalid request".into(),
            }
            .to_string(),
            "gopls error -32600: invalid request"
        );
        assert_eq!(
            ClientError::ProcessExited.to_string(),
            "gopls process exited unexpectedly"
        );
    }

    #[test]
    fn which_gopls_returns_not_found_or_path() {
        // This test is environment-dependent: it passes regardless of whether
        // gopls is installed, just verifying the function doesn't panic.
        let result = which_gopls();
        match result {
            Ok(path) => assert!(path.to_str().unwrap().contains("gopls")),
            Err(ClientError::NotFound) => {} // expected if not installed
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn invalid_workspace_fails() {
        let result = GoplsClient::start(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(matches!(result, Err(ClientError::InvalidWorkspace(_))));
    }
}
