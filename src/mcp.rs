//! MCP server over stdio — exposes clawmark as remember/recall tools
//!
//! Claude Desktop connects via stdio transport. The agent gets three
//! human-friendly tools that wrap clawmark's signal/tune operations.
//!
//! Tool descriptions guide the model toward proactive memory use —
//! recalling on startup, remembering during conversation.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use crate::db::{DatabaseManager, SignalEntry};

// =============================================================================
// MCP Protocol Types
// =============================================================================

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

fn success(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse { jsonrpc: "2.0".to_string(), id, result: Some(result), error: None }
}

fn error_response(id: Value, code: i64, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(), id, result: None,
        error: Some(json!({ "code": code, "message": message })),
    }
}

fn tool_result(id: Value, text: &str, is_error: bool) -> JsonRpcResponse {
    success(id, json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    }))
}

// =============================================================================
// Tool Definitions
// =============================================================================

fn tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "remember",
                "description": "Save something worth remembering for future sessions. Use this when you learn something important about the user — their preferences, decisions, client details, project context, or anything they would not want to repeat. Your future self will find it by meaning, not keywords. Signal more than you think you should — storage is free, forgetting is expensive.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The full detail. Write for a future you that knows nothing about this session. Include names, numbers, decisions, reasoning, context."
                        },
                        "gist": {
                            "type": "string",
                            "description": "A one-line summary for finding this later. Format: 'category: key insight'. Example: 'client: Maria — Q2 retention focus, $40K budget'"
                        },
                        "thread": {
                            "type": "string",
                            "description": "Optional. Short UUID of a previous memory to thread this to. Builds chains — prospect to client, draft to final, problem to solution."
                        }
                    },
                    "required": ["content", "gist"]
                }
            },
            {
                "name": "recall",
                "description": "Search your memories by meaning. Use this at the START of every conversation to check what you already know about the topic or the user. Also use it whenever you need context from previous sessions. The search finds related memories even when the words are different — 'budget priorities' finds memories about 'retention focus, $40K'. Use this proactively and often.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "What you are looking for. A topic, a name, a concept. Semantic search finds related memories even if the exact words differ."
                        },
                        "full": {
                            "type": "boolean",
                            "description": "If true, returns full content of each memory. Default false (gist summaries only)."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results. Default 10."
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "recall_recent",
                "description": "Get the most recent memories. Use this at the START of every conversation to see what happened in recent sessions. This is how you orient yourself — what was the user working on? What decisions were made? What context matters right now?",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Number of recent memories to return. Default 5."
                        },
                        "full": {
                            "type": "boolean",
                            "description": "If true, returns full content. Default false (gist summaries only)."
                        }
                    }
                }
            }
        ]
    })
}

// =============================================================================
// Tool Execution
// =============================================================================

fn execute_remember(db: &DatabaseManager, params: &Value) -> (String, bool) {
    let content = match params.get("content").and_then(|c| c.as_str()) {
        Some(c) => c,
        None => return ("Error: content is required".to_string(), true),
    };
    let gist = params.get("gist").and_then(|g| g.as_str());
    let thread = params.get("thread").and_then(|t| t.as_str());

    match db.signal_with_backend(content, gist, thread, None, None) {
        Ok(uuid) => (format!("Remembered ({})", uuid), false),
        Err(e) => (format!("Error: {}", e), true),
    }
}

fn execute_recall(db: &DatabaseManager, params: &Value) -> (String, bool) {
    let query = match params.get("query").and_then(|q| q.as_str()) {
        Some(q) => q,
        None => return ("Error: query is required".to_string(), true),
    };
    let full = params.get("full").and_then(|f| f.as_bool()).unwrap_or(false);
    let limit = params.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;

    // Semantic first, keyword fallback
    let results = match db.semantic_search(query, limit) {
        Ok(r) if !r.is_empty() => r,
        _ => match db.keyword_search(query, limit) {
            Ok(r) => r,
            Err(e) => return (format!("Error: {}", e), true),
        },
    };

    if results.is_empty() {
        return ("No memories found for that query.".to_string(), false);
    }

    (format_entries(&results, full, db), false)
}

fn execute_recall_recent(db: &DatabaseManager, params: &Value) -> (String, bool) {
    let limit = params.get("limit").and_then(|l| l.as_u64()).unwrap_or(5) as usize;
    let full = params.get("full").and_then(|f| f.as_bool()).unwrap_or(false);

    match db.recent(limit) {
        Ok(entries) if entries.is_empty() => {
            ("No memories yet. This is a fresh start.".to_string(), false)
        }
        Ok(entries) => (format_entries(&entries, full, db), false),
        Err(e) => (format!("Error: {}", e), true),
    }
}

fn format_entries(entries: &[SignalEntry], full: bool, db: &DatabaseManager) -> String {
    let mut lines = Vec::new();
    for e in entries {
        let ts = crate::shorten_ts(&e.created_at);
        let score_str = e.score.map(|s| format!(" ({:.2})", s)).unwrap_or_default();
        if full {
            let content = db.get_full_content(&e.signal_uuid)
                .ok().flatten().unwrap_or_default();
            lines.push(format!("{} | {} | {}{}\n  {}", &e.signal_uuid[..8], ts, e.gist, score_str, content));
        } else {
            lines.push(format!("{} | {} | {}{}", &e.signal_uuid[..8], ts, e.gist, score_str));
        }
    }
    lines.join("\n")
}

// =============================================================================
// MCP Server Loop
// =============================================================================

pub fn serve() {
    let db = match crate::get_db() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("[geniuz] Failed to open station: {}", e);
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let id = request.id.clone().unwrap_or(Value::Null);

        let response = match request.method.as_str() {
            "initialize" => {
                success(id, json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "geniuz",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }))
            }

            "notifications/initialized" => continue,

            "tools/list" => success(id, tool_definitions()),

            "tools/call" => {
                let tool_name = request.params.get("name")
                    .and_then(|n| n.as_str()).unwrap_or("");
                let arguments = request.params.get("arguments")
                    .cloned().unwrap_or(json!({}));

                let (text, is_error) = match tool_name {
                    "remember" => execute_remember(&db, &arguments),
                    "recall" => execute_recall(&db, &arguments),
                    "recall_recent" => execute_recall_recent(&db, &arguments),
                    _ => (format!("Unknown tool: {}", tool_name), true),
                };

                tool_result(id, &text, is_error)
            }

            _ => error_response(id, -32601, &format!("Method not found: {}", request.method)),
        };

        let json_str = serde_json::to_string(&response).unwrap_or_default();
        let _ = writeln!(stdout, "{}", json_str);
        let _ = stdout.flush();
    }
}
