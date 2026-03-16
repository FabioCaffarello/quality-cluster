//! LSP JSON-RPC protocol primitives.
//!
//! Encodes and decodes the LSP wire format (Content-Length header + JSON body)
//! used to communicate with `gopls` over stdio.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── JSON-RPC request ────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }

    /// Encode as an LSP wire message: `Content-Length: N\r\n\r\n{json}`.
    pub fn encode(&self) -> Vec<u8> {
        let body = serde_json::to_string(self).expect("request serialization cannot fail");
        encode_message(&body)
    }
}

/// A JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let body = serde_json::to_string(self).expect("notification serialization cannot fail");
        encode_message(&body)
    }
}

// ── JSON-RPC response ───────────────────────────────────────────────────────

/// A JSON-RPC 2.0 response message.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC error object.
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[allow(dead_code)]
    pub data: Option<Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LSP error {}: {}", self.code, self.message)
    }
}

// ── Wire encoding / decoding ────────────────────────────────────────────────

/// Encode a JSON body into LSP wire format.
fn encode_message(body: &str) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut buf = Vec::with_capacity(header.len() + body.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(body.as_bytes());
    buf
}

/// Read one LSP message from a reader.
///
/// Parses the `Content-Length` header, reads exactly that many bytes, and
/// deserializes the JSON body.
pub fn read_message(reader: &mut impl std::io::BufRead) -> std::io::Result<JsonRpcResponse> {
    // Read headers until empty line.
    let mut content_length: Option<usize> = None;
    loop {
        let mut header_line = String::new();
        let n = reader.read_line(&mut header_line)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "gopls closed stdout before sending a complete message",
            ));
        }
        let trimmed = header_line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("Content-Length:") {
            content_length = val.trim().parse().ok();
        }
    }

    let length = content_length.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing Content-Length header in LSP message",
        )
    })?;

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    serde_json::from_slice(&body).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid JSON in LSP response: {e}"),
        )
    })
}

// ── LSP data types (subset used by the bridge) ─────────────────────────────

/// LSP Position (0-indexed line and character).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// LSP Location (URI + range).
#[derive(Debug, Clone, Deserialize)]
pub struct LspLocation {
    pub uri: String,
    pub range: Range,
}

/// LSP Range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// LSP TextDocumentIdentifier.
#[derive(Debug, Serialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

/// LSP TextDocumentPositionParams.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// LSP ReferenceParams.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub context: ReferenceContext,
}

/// LSP ReferenceContext.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceContext {
    pub include_declaration: bool,
}

/// LSP Hover result.
#[derive(Debug, Deserialize)]
pub struct HoverResult {
    pub contents: HoverContents,
}

/// LSP MarkupContent or plain string (gopls returns MarkupContent).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum HoverContents {
    Markup { kind: String, value: String },
    Plain(String),
}

impl HoverContents {
    pub fn text(&self) -> &str {
        match self {
            HoverContents::Markup { value, .. } => value,
            HoverContents::Plain(s) => s,
        }
    }
}

/// Convert a file path to a `file://` URI.
pub fn path_to_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_string()
    } else {
        format!("file://{path}")
    }
}

/// Extract the file path from a `file://` URI.
pub fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let req = JsonRpcRequest::new(1, "initialize", None);
        let encoded = req.encode();
        let s = String::from_utf8(encoded.clone()).unwrap();
        assert!(s.starts_with("Content-Length: "));
        assert!(s.contains("\"jsonrpc\":\"2.0\""));
        assert!(s.contains("\"id\":1"));

        // Decode it back
        let mut cursor = std::io::Cursor::new(encoded);
        let mut reader = std::io::BufReader::new(&mut cursor);
        let resp = read_message(&mut reader).unwrap();
        // It will parse as a response (id=1, no result/error)
        assert_eq!(resp.id, Some(1));
    }

    #[test]
    fn notification_has_no_id() {
        let notif = JsonRpcNotification::new("initialized", None);
        let body = serde_json::to_string(&notif).unwrap();
        assert!(!body.contains("\"id\""));
        assert!(body.contains("\"initialized\""));
    }

    #[test]
    fn path_uri_conversions() {
        assert_eq!(path_to_uri("/foo/bar.go"), "file:///foo/bar.go");
        assert_eq!(path_to_uri("file:///already/uri"), "file:///already/uri");
        assert_eq!(uri_to_path("file:///foo/bar.go"), "/foo/bar.go");
        assert_eq!(uri_to_path("/no/prefix"), "/no/prefix");
    }

    #[test]
    fn missing_content_length_is_error() {
        let bad = b"\r\n{\"jsonrpc\":\"2.0\"}";
        let mut reader = std::io::BufReader::new(&bad[..]);
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn hover_contents_text() {
        let markup = HoverContents::Markup {
            kind: "markdown".into(),
            value: "func Foo()".into(),
        };
        assert_eq!(markup.text(), "func Foo()");

        let plain = HoverContents::Plain("hello".into());
        assert_eq!(plain.text(), "hello");
    }
}
