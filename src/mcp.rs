//! MCP (Model Context Protocol) server. JSON-RPC 2.0 over stdio.
//! Exposes kova tools to external AI clients (Claude Desktop, etc).
//! f174=mcp_tools_list, f175=mcp_handle_request, f176=mcp_stdio_loop.
//! t112=McpRequest, t113=McpResponse.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::{json, Value};

use crate::tools::{self, t103, TOOLS};

/// Protocol version we support.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name.
const SERVER_NAME: &str = "kova";

/// Server version (matches Cargo.toml).
const SERVER_VERSION: &str = "0.2.0";

// ── MCP Types ────────────────────────────────────────────

/// t112=McpRequest. Parsed JSON-RPC 2.0 request.
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct t112 {
    pub id: Value,
    pub method: String,
    pub params: Value,
}

/// t113=McpResponse. JSON-RPC 2.0 response.
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct t113 {
    pub id: Value,
    pub result: Option<Value>,
    pub error: Option<T210>,
}

/// MCP error payload.
#[derive(Debug)]
pub struct T210 {
    pub code: i64,
    pub message: String,
}

impl t113 {
    fn ok(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: Value, code: i64, message: String) -> Self {
        Self {
            id,
            result: None,
            error: Some(T210 { code, message }),
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = json!({"jsonrpc": "2.0", "id": self.id});
        if let Some(ref result) = self.result {
            obj["result"] = result.clone();
        }
        if let Some(ref error) = self.error {
            obj["error"] = json!({
                "code": error.code,
                "message": error.message,
            });
        }
        obj
    }
}

// ── JSON-RPC error codes ─────────────────────────────────

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

// ── Tool Schema Generation ───────────────────────────────

/// Build JSON Schema `properties` and `required` from tool params.
fn tool_input_schema(tool: &tools::t101) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for p in tool.params {
        let json_type = match p.param_type {
            "number" => "number",
            "boolean" => "boolean",
            _ => "string",
        };
        properties.insert(
            p.name.to_string(),
            json!({
                "type": json_type,
                "description": p.description,
            }),
        );
        if p.required {
            required.push(Value::String(p.name.to_string()));
        }
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

// ── f174: mcp_tools_list ─────────────────────────────────

/// f174=mcp_tools_list. Return all kova tools in MCP format.
pub fn f174() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": tool_input_schema(t),
            })
        })
        .collect()
}

// ── f175: mcp_handle_request ─────────────────────────────

/// f175=mcp_handle_request. Parse JSON-RPC request, dispatch, return JSON-RPC response string.
/// Returns empty string for notifications (no id) — caller should skip writing those.
pub fn f175(req: &str, project_dir: &Path) -> String {
    let parsed: Value = match serde_json::from_str(req) {
        Ok(v) => v,
        Err(_) => {
            let resp = t113::err(Value::Null, PARSE_ERROR, "Parse error".into());
            return serde_json::to_string(&resp.to_json()).unwrap_or_default();
        }
    };

    // Validate jsonrpc version field.
    if parsed.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        let resp = t113::err(Value::Null, INVALID_REQUEST, "Missing or invalid jsonrpc version".into());
        return serde_json::to_string(&resp.to_json()).unwrap_or_default();
    }

    let method = match parsed.get("method").and_then(|m| m.as_str()) {
        Some(m) => m.to_string(),
        None => {
            let id = parsed.get("id").cloned().unwrap_or(Value::Null);
            let resp = t113::err(id, INVALID_REQUEST, "Missing method".into());
            return serde_json::to_string(&resp.to_json()).unwrap_or_default();
        }
    };

    // Notifications have no "id" field — per JSON-RPC 2.0, no response should be sent.
    let is_notification = parsed.get("id").is_none_or(|v| v.is_null());
    if is_notification && method.starts_with("notifications/") {
        return String::new();
    }

    let id = parsed.get("id").cloned().unwrap_or(Value::Null);
    let params = parsed.get("params").cloned().unwrap_or(Value::Null);

    let resp = dispatch_method(&method, &params, &id, project_dir);
    serde_json::to_string(&resp.to_json()).unwrap_or_default()
}

/// Route MCP method to handler.
fn dispatch_method(method: &str, params: &Value, id: &Value, project_dir: &Path) -> t113 {
    match method {
        "initialize" => handle_initialize(id),
        "notifications/initialized" => {
            // Client acknowledgment — no response needed for notifications,
            // but we return success since we got an id.
            t113::ok(id.clone(), json!({}))
        }
        "tools/list" => handle_tools_list(id),
        "tools/call" => handle_tools_call(id, params, project_dir),
        _ => t113::err(id.clone(), METHOD_NOT_FOUND, format!("Unknown method: {}", method)),
    }
}

/// Handle "initialize" — return server capabilities.
fn handle_initialize(id: &Value) -> t113 {
    t113::ok(
        id.clone(),
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            }
        }),
    )
}

/// Handle "tools/list" — return tool definitions.
fn handle_tools_list(id: &Value) -> t113 {
    t113::ok(id.clone(), json!({"tools": f174()}))
}

/// Handle "tools/call" — dispatch to kova tool, return result.
fn handle_tools_call(id: &Value, params: &Value, project_dir: &Path) -> t113 {
    let name = match params.get("name").and_then(|n| n.as_str()) {
        Some(n) => n,
        None => {
            return t113::err(id.clone(), INVALID_PARAMS, "Missing tool name".into());
        }
    };

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    // Convert MCP arguments to kova t103 (ToolCall).
    let mut args = HashMap::new();
    if let Some(obj) = arguments.as_object() {
        for (k, v) in obj {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            args.insert(k.clone(), val);
        }
    }

    let call = t103 {
        tool: name.to_string(),
        args,
    };

    let result = tools::f141(&call, project_dir);

    if result.success {
        t113::ok(
            id.clone(),
            json!({
                "content": [{
                    "type": "text",
                    "text": result.output,
                }]
            }),
        )
    } else {
        t113::ok(
            id.clone(),
            json!({
                "content": [{
                    "type": "text",
                    "text": result.output,
                }],
                "isError": true,
            }),
        )
    }
}

// ── f176: mcp_stdio_loop ─────────────────────────────────

/// f176=mcp_stdio_loop. Read stdin line by line, handle each as MCP request, write to stdout.
pub fn f176(project_dir: &Path) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = f175(trimmed, project_dir);
        // Skip empty responses (notifications don't get a reply per JSON-RPC 2.0).
        if !response.is_empty() {
            let _ = writeln!(stdout, "{}", response);
            let _ = stdout.flush();
        }
    }
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// f174=mcp_tools_list returns expected tools.
    #[test]
    fn f174_tools_list_contains_all_tools() {
        let tools = f174();
        assert!(!tools.is_empty());

        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(names.contains(&"read_file"), "missing read_file");
        assert!(names.contains(&"write_file"), "missing write_file");
        assert!(names.contains(&"edit_file"), "missing edit_file");
        assert!(names.contains(&"exec"), "missing exec");
        assert!(names.contains(&"glob"), "missing glob");
        assert!(names.contains(&"grep"), "missing grep");
        assert!(names.contains(&"memory_write"), "missing memory_write");

        // Each tool has inputSchema with type=object
        for t in &tools {
            let schema = t.get("inputSchema").unwrap();
            assert_eq!(schema.get("type").unwrap(), "object");
            assert!(schema.get("properties").is_some());
            assert!(schema.get("required").is_some());
        }
    }

    /// f175=mcp_handle_request dispatches tools/call correctly.
    #[test]
    fn f175_dispatch_read_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let test_file = tmp.path().join("hello.txt");
        std::fs::write(&test_file, "hello world").unwrap();

        let req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {
                    "path": test_file.to_str().unwrap(),
                }
            }
        });

        let resp_str = f175(&serde_json::to_string(&req).unwrap(), tmp.path());
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 3);
        assert!(resp["error"].is_null());

        let content = &resp["result"]["content"][0];
        assert_eq!(content["type"], "text");
        assert!(content["text"].as_str().unwrap().contains("hello world"));
    }

    /// f175=mcp_handle_request returns capabilities on initialize.
    #[test]
    fn f175_initialize_returns_capabilities() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05"
            }
        });

        let resp_str = f175(
            &serde_json::to_string(&req).unwrap(),
            Path::new("/tmp"),
        );
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);

        let result = &resp["result"];
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "kova");
        assert_eq!(result["serverInfo"]["version"], "0.2.0");
    }

    /// f175=mcp_handle_request returns tools list.
    #[test]
    fn f175_tools_list_returns_tools() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
        });

        let resp_str = f175(
            &serde_json::to_string(&req).unwrap(),
            Path::new("/tmp"),
        );
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["id"], 2);
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        assert!(tools.iter().any(|t| t["name"] == "read_file"));
    }

    /// f175=mcp_handle_request returns error for unknown method.
    #[test]
    fn f175_unknown_method_returns_error() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "bogus/method",
        });

        let resp_str = f175(
            &serde_json::to_string(&req).unwrap(),
            Path::new("/tmp"),
        );
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["id"], 99);
        assert!(resp["error"].is_object());
        assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);
    }

    /// f175=mcp_handle_request returns parse error for invalid JSON.
    #[test]
    fn f175_invalid_json_returns_parse_error() {
        let resp_str = f175("not json at all", Path::new("/tmp"));
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert!(resp["error"].is_object());
        assert_eq!(resp["error"]["code"], PARSE_ERROR);
    }

    /// f175=mcp_handle_request returns isError for failed tool call.
    #[test]
    fn f175_tool_error_returns_is_error() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {
                    "path": "/nonexistent/path/that/does/not/exist.txt",
                }
            }
        });

        let resp_str = f175(
            &serde_json::to_string(&req).unwrap(),
            Path::new("/tmp"),
        );
        let resp: Value = serde_json::from_str(&resp_str).unwrap();

        assert_eq!(resp["id"], 5);
        // Tool errors come back as result with isError=true (MCP convention).
        assert_eq!(resp["result"]["isError"], true);
    }

    /// f175 rejects requests missing jsonrpc version.
    #[test]
    fn f175_missing_jsonrpc_version_returns_error() {
        let req = json!({
            "id": 10,
            "method": "tools/list",
        });
        let resp_str = f175(&serde_json::to_string(&req).unwrap(), Path::new("/tmp"));
        let resp: Value = serde_json::from_str(&resp_str).unwrap();
        assert!(resp["error"].is_object());
        assert_eq!(resp["error"]["code"], INVALID_REQUEST);
        assert!(resp["error"]["message"].as_str().unwrap().contains("jsonrpc"));
    }

    /// f175 returns empty string for notifications (no id).
    #[test]
    fn f175_notification_returns_empty() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        });
        let resp_str = f175(&serde_json::to_string(&req).unwrap(), Path::new("/tmp"));
        assert!(resp_str.is_empty(), "notifications should produce no response");
    }

    /// f175 tools/call with missing required args returns isError.
    #[test]
    fn f175_tools_call_missing_args_returns_error() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {}
            }
        });
        let resp_str = f175(&serde_json::to_string(&req).unwrap(), Path::new("/tmp"));
        let resp: Value = serde_json::from_str(&resp_str).unwrap();
        assert_eq!(resp["id"], 7);
        assert!(resp["result"]["isError"] == true || resp["error"].is_object());
    }
}