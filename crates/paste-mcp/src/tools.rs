use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Utc};
use paste_core::{Database, ItemType, NewClipboardPayload, SearchFilter};
use serde_json::{json, Value};

/// Returns the JSON Schema definitions for all 11 Paste 7.0.5 MCP tools.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "search_saved_items",
            "description": "Search clipboard history and pinboards with full-text query and structured filters (type, source app, device, title, pinboard, date range).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Full-text search query across title, text, app name, and URL." },
                    "type": { "type": "string", "enum": ["text", "link", "image", "color", "file"], "description": "Filter by item type." },
                    "app": { "type": "string", "description": "Filter by source application name or bundle/executable id." },
                    "device": { "type": "string", "description": "Filter by originating device name or id." },
                    "title": { "type": "string", "description": "Filter by item title substring." },
                    "pinboard_id": { "type": "string", "description": "Restrict results to a specific pinboard UUID." },
                    "date_from": { "type": "string", "description": "ISO-8601 start timestamp or Unix milliseconds." },
                    "date_to": { "type": "string", "description": "ISO-8601 end timestamp or Unix milliseconds." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20, "description": "Maximum number of items to return (max 100)." }
                }
            }
        }),
        json!({
            "name": "read_item",
            "description": "Retrieve the complete content of a saved clipboard item by ID (verbatim text, image base64, hex color, file paths, URL).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "UUID of the saved item." }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "create_item",
            "description": "Create a new saved clip from text, with automatic detection for colors and links, optionally pinning it to a pinboard.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Text, URL, or color string to store." },
                    "title": { "type": "string", "description": "Optional custom title." },
                    "pinboard_id": { "type": "string", "description": "Optional pinboard UUID to pin the new item into." }
                },
                "required": ["content"]
            }
        }),
        json!({
            "name": "delete_saved_item",
            "description": "Irreversibly delete a saved clipboard item. Requires confirm=true on the second step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "UUID of the item to delete." },
                    "confirm": { "type": "boolean", "description": "Pass true to confirm permanent deletion." }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "list_pinboards",
            "description": "List all pinboards with their UUIDs, names, colors, tab ordering, and item counts.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "create_pinboard",
            "description": "Create a new pinboard collection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Display name for the pinboard." },
                    "color_hex": { "type": "string", "description": "Optional hex color (e.g. #4A90E2)." }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "rename_pinboard",
            "description": "Rename an existing pinboard.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Pinboard UUID." },
                    "name": { "type": "string", "description": "New name for the pinboard." }
                },
                "required": ["id", "name"]
            }
        }),
        json!({
            "name": "delete_pinboard",
            "description": "Delete a pinboard (items remain in Clipboard History). Requires confirm=true on the second step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Pinboard UUID to delete." },
                    "confirm": { "type": "boolean", "description": "Pass true to confirm pinboard deletion." }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "add_item_to_pinboard",
            "description": "Pin an existing clipboard item to a pinboard (idempotent).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "item_id": { "type": "string", "description": "Item UUID." },
                    "pinboard_id": { "type": "string", "description": "Target pinboard UUID." }
                },
                "required": ["item_id", "pinboard_id"]
            }
        }),
        json!({
            "name": "remove_item_from_pinboard",
            "description": "Unpin an item from its pinboard while keeping it in Clipboard History.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "item_id": { "type": "string", "description": "Item UUID." },
                    "pinboard_id": { "type": "string", "description": "Optional pinboard UUID." }
                },
                "required": ["item_id"]
            }
        }),
        json!({
            "name": "update_item",
            "description": "Update the title and/or text content of an existing saved item.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Item UUID." },
                    "title": { "type": "string", "description": "New custom title." },
                    "content": { "type": "string", "description": "New text content." }
                },
                "required": ["id"]
            }
        }),
    ]
}

pub fn execute_tool(db: &Database, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "search_saved_items" => tool_search_saved_items(db, args),
        "read_item" => tool_read_item(db, args),
        "create_item" => tool_create_item(db, args),
        "delete_saved_item" => tool_delete_saved_item(db, args),
        "list_pinboards" => tool_list_pinboards(db),
        "create_pinboard" => tool_create_pinboard(db, args),
        "rename_pinboard" => tool_rename_pinboard(db, args),
        "delete_pinboard" => tool_delete_pinboard(db, args),
        "add_item_to_pinboard" => tool_add_item_to_pinboard(db, args),
        "remove_item_from_pinboard" => tool_remove_item_from_pinboard(db, args),
        "update_item" => tool_update_item(db, args),
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn tool_search_saved_items(db: &Database, args: &Value) -> Result<Value, String> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .clamp(1, 100) as usize;

    let item_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .and_then(ItemType::from_str_filter);

    let filter = SearchFilter {
        query: args.get("query").and_then(|v| v.as_str()).map(ToString::to_string),
        item_type,
        source_app: args.get("app").and_then(|v| v.as_str()).map(ToString::to_string),
        device: args.get("device").and_then(|v| v.as_str()).map(ToString::to_string),
        title: args.get("title").and_then(|v| v.as_str()).map(ToString::to_string),
        pinboard_id: args.get("pinboard_id").and_then(|v| v.as_str()).map(ToString::to_string),
        date_from_ms: args.get("date_from").and_then(parse_date_arg),
        date_to_ms: args.get("date_to").and_then(parse_date_arg),
        limit,
    };

    let items = db.search_items(&filter).map_err(|e| e.to_string())?;
    let summaries: Vec<Value> = items
        .into_iter()
        .map(|it| {
            json!({
                "id": it.identifier,
                "title": it.title,
                "type": it.item_type.as_str(),
                "snippet": it.preview.snippet,
                "source_app": it.source_app_name,
                "device": it.device_name,
                "pinboard_id": it.pinboard_id,
                "is_confidential": it.is_confidential,
                "timestamp": it.timestamp,
            })
        })
        .collect();

    Ok(json!({
        "count": summaries.len(),
        "items": summaries,
    }))
}

fn tool_read_item(db: &Database, args: &Value) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let item = db
        .get_item(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("item '{id}' not found"))?;
    let data = db.get_item_data(id).map_err(|e| e.to_string())?;

    let image_b64 = data
        .as_ref()
        .and_then(|d| d.image_png.as_ref())
        .map(|bytes| BASE64.encode(bytes));

    Ok(json!({
        "id": item.identifier,
        "title": item.title,
        "type": item.item_type.as_str(),
        "checksum": item.checksum,
        "source_app": item.source_app_name,
        "device": item.device_name,
        "pinboard_id": item.pinboard_id,
        "is_confidential": item.is_confidential,
        "timestamp": item.timestamp,
        "created_at": item.created_at,
        "updated_at": item.updated_at,
        "preview": item.preview,
        "content": data.as_ref().and_then(|d| d.plain_text.clone()),
        "file_paths": data.as_ref().map(|d| d.file_paths.clone()).unwrap_or_default(),
        "image_png_base64": image_b64,
    }))
}

fn tool_create_item(db: &Database, args: &Value) -> Result<Value, String> {
    let content = req_str(args, "content")?.to_string();
    let title = args.get("title").and_then(|v| v.as_str()).map(ToString::to_string);
    let pinboard_id = args
        .get("pinboard_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let (item, is_new) = db
        .ingest_clipboard_payload(NewClipboardPayload {
            plain_text: Some(content),
            custom_title: title,
            pinboard_id,
            source_app_id: Some("mcp-client".to_string()),
            source_app_name: Some("MCP Client".to_string()),
            source_app_color: Some("#6C5CE7".to_string()),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "created": is_new,
        "deduplicated": !is_new,
        "item": {
            "id": item.identifier,
            "title": item.title,
            "type": item.item_type.as_str(),
            "pinboard_id": item.pinboard_id,
            "checksum": item.checksum,
        }
    }))
}

fn tool_delete_saved_item(db: &Database, args: &Value) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let confirm = args.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);

    let item = db
        .get_item(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("item '{id}' not found"))?;

    if !confirm {
        return Ok(json!({
            "status": "confirmation_required",
            "message": format!("Are you sure you want to permanently delete item '{}' ({})? Call delete_saved_item again with confirm=true to proceed.", item.title, item.identifier),
            "id": item.identifier,
            "title": item.title,
        }));
    }

    let deleted = db.delete_item(id).map_err(|e| e.to_string())?;
    Ok(json!({
        "deleted": deleted,
        "id": id,
    }))
}

fn tool_list_pinboards(db: &Database) -> Result<Value, String> {
    let list = db.list_pinboards().map_err(|e| e.to_string())?;
    Ok(json!({
        "count": list.len(),
        "pinboards": list,
    }))
}

fn tool_create_pinboard(db: &Database, args: &Value) -> Result<Value, String> {
    let name = req_str(args, "name")?;
    let color = args.get("color_hex").and_then(|v| v.as_str());
    let pb = db.create_pinboard(name, color).map_err(|e| e.to_string())?;
    Ok(json!(pb))
}

fn tool_rename_pinboard(db: &Database, args: &Value) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let name = req_str(args, "name")?;
    let pb = db.rename_pinboard(id, name).map_err(|e| e.to_string())?;
    Ok(json!(pb))
}

fn tool_delete_pinboard(db: &Database, args: &Value) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let confirm = args.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);

    let pb = db
        .get_pinboard(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("pinboard '{id}' not found"))?;

    if !confirm {
        return Ok(json!({
            "status": "confirmation_required",
            "message": format!("Are you sure you want to delete pinboard '{}' ({} items will remain in Clipboard History)? Call delete_pinboard again with confirm=true.", pb.name, pb.item_count),
            "id": pb.identifier,
            "name": pb.name,
        }));
    }

    let deleted = db.delete_pinboard(id).map_err(|e| e.to_string())?;
    Ok(json!({
        "deleted": deleted,
        "id": id,
    }))
}

fn tool_add_item_to_pinboard(db: &Database, args: &Value) -> Result<Value, String> {
    let item_id = req_str(args, "item_id")?;
    let pinboard_id = req_str(args, "pinboard_id")?;
    let item = db
        .add_item_to_pinboard(item_id, pinboard_id)
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "pinned": true,
        "item_id": item.identifier,
        "pinboard_id": item.pinboard_id,
    }))
}

fn tool_remove_item_from_pinboard(db: &Database, args: &Value) -> Result<Value, String> {
    let item_id = req_str(args, "item_id")?;
    let item = db
        .remove_item_from_pinboard(item_id)
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "unpinned": true,
        "item_id": item.identifier,
        "pinboard_id": item.pinboard_id,
    }))
}

fn tool_update_item(db: &Database, args: &Value) -> Result<Value, String> {
    let id = req_str(args, "id")?;
    let title = args.get("title").and_then(|v| v.as_str());
    let content = args.get("content").and_then(|v| v.as_str());
    if title.is_none() && content.is_none() {
        return Err("at least one of 'title' or 'content' must be provided".to_string());
    }
    let item = db
        .update_item(id, title, content)
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "updated": true,
        "id": item.identifier,
        "title": item.title,
        "type": item.item_type.as_str(),
    }))
}

fn req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("missing or empty required parameter '{key}'"))
}

fn parse_date_arg(val: &Value) -> Option<i64> {
    if let Some(n) = val.as_i64() {
        return Some(n);
    }
    if let Some(s) = val.as_str() {
        if let Ok(n) = s.trim().parse::<i64>() {
            return Some(n);
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
            return Some(dt.with_timezone(&Utc).timestamp_millis());
        }
    }
    None
}
