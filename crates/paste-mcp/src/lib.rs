pub mod protocol;
pub mod tools;

use paste_core::Database;
use serde_json::{json, Value};

use protocol::{JsonRpcRequest, JsonRpcResponse};
use tools::{execute_tool, tool_definitions};

/// Handles a single JSON-RPC 2.0 MCP request line against a `Database`.
/// Returns `None` for notifications that expect no reply.
pub fn handle_jsonrpc_request(db: &Database, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let is_notification = req.id.is_none() || req.method.starts_with("notifications/");
    let id = req.id.unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => Some(JsonRpcResponse::ok(
            id,
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "new-paste-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )),
        "notifications/initialized" => {
            if is_notification {
                None
            } else {
                Some(JsonRpcResponse::ok(id, json!({})))
            }
        }
        "ping" => Some(JsonRpcResponse::ok(id, json!({}))),
        "tools/list" => Some(JsonRpcResponse::ok(
            id,
            json!({
                "tools": tool_definitions()
            }),
        )),
        "tools/call" => {
            let tool_name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            match execute_tool(db, tool_name, &arguments) {
                Ok(result_val) => {
                    let pretty = serde_json::to_string_pretty(&result_val)
                        .unwrap_or_else(|_| result_val.to_string());
                    Some(JsonRpcResponse::ok(
                        id,
                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": pretty
                                }
                            ],
                            "isError": false
                        }),
                    ))
                }
                Err(err_msg) => Some(JsonRpcResponse::ok(
                    id,
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": err_msg
                            }
                        ],
                        "isError": true
                    }),
                )),
            }
        }
        other => {
            if is_notification {
                None
            } else {
                Some(JsonRpcResponse::err(
                    id,
                    -32601,
                    format!("method not found: {other}"),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(db: &Database, id: i64, method: &str, params: Value) -> Value {
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(id)),
            method: method.to_string(),
            params,
        };
        let resp = handle_jsonrpc_request(db, req).expect("response expected");
        serde_json::to_value(resp).unwrap()
    }

    fn call_tool(db: &Database, name: &str, args: Value) -> Value {
        let resp = call(
            db,
            1,
            "tools/call",
            json!({ "name": name, "arguments": args }),
        );
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(resp["result"]["isError"], json!(false), "tool {name} failed: {text}");
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn test_mcp_initialize_and_list_11_tools() {
        let db = Database::open_in_memory().unwrap();
        let init = call(&db, 1, "initialize", json!({}));
        assert_eq!(init["result"]["serverInfo"]["name"], "new-paste-mcp");

        let tools_resp = call(&db, 2, "tools/list", json!({}));
        let tools = tools_resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 11);
    }

    #[test]
    fn test_mcp_full_crud_and_two_step_deletions() {
        let db = Database::open_in_memory().unwrap();

        // 1. Create pinboard
        let pb = call_tool(&db, "create_pinboard", json!({ "name": "API Keys", "color_hex": "#FF5733" }));
        let pb_id = pb["identifier"].as_str().unwrap().to_string();

        // 2. Rename pinboard
        let pb2 = call_tool(&db, "rename_pinboard", json!({ "id": pb_id, "name": "Docs" }));
        assert_eq!(pb2["name"], "Docs");

        // 3. List pinboards
        let list = call_tool(&db, "list_pinboards", json!({}));
        assert!(list["count"].as_u64().unwrap() >= 4);

        // 4. Create item
        let created = call_tool(&db, "create_item", json!({ "content": "https://docs.rs/rusqlite" }));
        let item_id = created["item"]["id"].as_str().unwrap().to_string();
        assert_eq!(created["item"]["type"], "link");

        // 5. Add & remove from pinboard
        let pinned = call_tool(&db, "add_item_to_pinboard", json!({ "item_id": item_id, "pinboard_id": pb_id }));
        assert_eq!(pinned["pinned"], true);

        // 6. Search saved items
        let found = call_tool(&db, "search_saved_items", json!({ "query": "rusqlite", "type": "link", "pinboard_id": pb_id }));
        assert_eq!(found["count"], 1);

        // 7. Update item & read item
        call_tool(&db, "update_item", json!({ "id": item_id, "title": "Rusqlite Docs", "content": "https://docs.rs/rusqlite/latest" }));
        let read = call_tool(&db, "read_item", json!({ "id": item_id }));
        assert_eq!(read["title"], "Rusqlite Docs");
        assert_eq!(read["content"], "https://docs.rs/rusqlite/latest");

        // 8. Unpin item
        let unpinned = call_tool(&db, "remove_item_from_pinboard", json!({ "item_id": item_id }));
        assert_eq!(unpinned["unpinned"], true);

        // 9. Two-step delete item
        let del_step1 = call_tool(&db, "delete_saved_item", json!({ "id": item_id }));
        assert_eq!(del_step1["status"], "confirmation_required");
        let del_step2 = call_tool(&db, "delete_saved_item", json!({ "id": item_id, "confirm": true }));
        assert_eq!(del_step2["deleted"], true);

        // 10. Two-step delete pinboard
        let pb_del1 = call_tool(&db, "delete_pinboard", json!({ "id": pb_id }));
        assert_eq!(pb_del1["status"], "confirmation_required");
        let pb_del2 = call_tool(&db, "delete_pinboard", json!({ "id": pb_id, "confirm": true }));
        assert_eq!(pb_del2["deleted"], true);
    }
}
