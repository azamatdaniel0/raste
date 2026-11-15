use std::io::{self, BufRead, Write};

use paste_core::Database;
use paste_mcp::handle_jsonrpc_request;
use paste_mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use serde_json::Value;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();

    let db_path = Database::default_db_path();
    let db = match Database::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            eprintln!("Failed to open new-paste database at {}: {err}", db_path.display());
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line_res in stdin.lock().lines() {
        let Ok(line) = line_res else {
            break;
        };
        let trimmed = line.trim().trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => {
                if let Some(resp) = handle_jsonrpc_request(&db, req) {
                    if let Ok(json_str) = serde_json::to_string(&resp) {
                        let _ = writeln!(out, "{json_str}");
                        let _ = out.flush();
                    }
                }
            }
            Err(err) => {
                let resp = JsonRpcResponse::err(
                    Value::Null,
                    -32700,
                    format!("parse error: {err}"),
                );
                if let Ok(json_str) = serde_json::to_string(&resp) {
                    let _ = writeln!(out, "{json_str}");
                    let _ = out.flush();
                }
            }
        }
    }
}
