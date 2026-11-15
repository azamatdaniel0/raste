pub mod error;
pub mod models;
pub mod picks;
pub mod processor;
pub mod search;
pub mod stack;
pub mod storage;

pub use error::{PasteError, Result};
pub use models::{
    ApplicationEntity, DeviceEntity, DeviceType, HistoryCapacity, ItemDataEntity, ItemEntity,
    ItemPreview, ItemType, NewClipboardPayload, Pinboard, Preferences,
};
pub use picks::{PickRecommendation, PicksEngine};
pub use search::SearchFilter;
pub use stack::{PasteStack, StackDirection};
pub use storage::Database;

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{ImageBuffer, ImageFormat, Rgba};

    use super::*;
    use crate::processor::{
        classify_and_build_preview, is_secret_or_confidential, parse_color, parse_url,
        remove_solid_background,
    };

    #[test]
    fn test_color_and_url_classification() {
        let (hex, rgba, _) = parse_color("#4a90e2").expect("parse hex");
        assert_eq!(hex, "#4A90E2");
        assert_eq!(rgba, [0x4A, 0x90, 0xE2, 255]);

        let (hex2, rgba2, _) = parse_color("rgb(255, 87, 51)").expect("parse rgb");
        assert_eq!(hex2, "#FF5733");
        assert_eq!(rgba2, [255, 87, 51, 255]);

        let (url, domain, _) = parse_url("https://www.rust-lang.org/learn").expect("parse url");
        assert_eq!(url, "https://www.rust-lang.org/learn");
        assert_eq!(domain, "rust-lang.org");

        let payload = NewClipboardPayload {
            plain_text: Some("#FF5733".to_string()),
            ..Default::default()
        };
        let (kind, title, _, is_secret) = classify_and_build_preview(&payload, true);
        assert_eq!(kind, ItemType::Color);
        assert_eq!(title, "#FF5733");
        assert!(!is_secret);
    }

    #[test]
    fn test_secret_detector() {
        assert!(is_secret_or_confidential("sk-proj-abc12345678901234567890"));
        assert!(is_secret_or_confidential("ghp_1234567890abcdefghij1234567890"));
        assert!(is_secret_or_confidential("AKIAIOSFODNN7EXAMPLE"));
        assert!(is_secret_or_confidential("-----BEGIN RSA PRIVATE KEY-----\nMIIE..."));
        assert!(!is_secret_or_confidential("Hello, world! This is normal text."));
        assert!(!is_secret_or_confidential("https://example.com/some/very/long/path"));
    }

    #[test]
    fn test_sha256_deduplication_and_cascade_delete() {
        let db = Database::open_in_memory().expect("open memory db");

        let (item1, is_new1) = db
            .ingest_clipboard_payload(NewClipboardPayload {
                plain_text: Some("cargo build --release".to_string()),
                source_app_id: Some("Code.exe".to_string()),
                source_app_name: Some("VS Code".to_string()),
                ..Default::default()
            })
            .expect("ingest 1");
        assert!(is_new1);

        std::thread::sleep(std::time::Duration::from_millis(5));

        let (item2, is_new2) = db
            .ingest_clipboard_payload(NewClipboardPayload {
                plain_text: Some("cargo build --release".to_string()),
                source_app_id: Some("WindowsTerminal.exe".to_string()),
                source_app_name: Some("Terminal".to_string()),
                ..Default::default()
            })
            .expect("ingest 2");
        assert!(!is_new2);
        assert_eq!(item1.identifier, item2.identifier);
        assert!(item2.timestamp >= item1.timestamp);

        assert!(db.get_item_data(&item1.identifier).unwrap().is_some());
        assert!(db.delete_item(&item1.identifier).unwrap());
        assert!(db.get_item(&item1.identifier).unwrap().is_none());
        assert!(db.get_item_data(&item1.identifier).unwrap().is_none());
    }

    #[test]
    fn test_search_filters_and_pinboards() {
        let db = Database::open_in_memory().expect("open memory db");
        let pb = db.create_pinboard("My Snippets", Some("#FF5733")).unwrap();

        let (link_item, _) = db
            .ingest_clipboard_payload(NewClipboardPayload {
                plain_text: Some("https://crates.io/crates/rusqlite".to_string()),
                source_app_id: Some("zen.exe".to_string()),
                source_app_name: Some("Zen".to_string()),
                ..Default::default()
            })
            .unwrap();

        let (code_item, _) = db
            .ingest_clipboard_payload(NewClipboardPayload {
                plain_text: Some("fn main() { println!(\"hi\"); }".to_string()),
                source_app_id: Some("Code.exe".to_string()),
                source_app_name: Some("VS Code".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.add_item_to_pinboard(&code_item.identifier, &pb.identifier)
            .unwrap();

        let links = db
            .search_items(&SearchFilter {
                item_type: Some(ItemType::Link),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].identifier, link_item.identifier);

        let pinned = db
            .search_items(&SearchFilter {
                pinboard_id: Some(pb.identifier.clone()),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].identifier, code_item.identifier);

        // Deleting pinboard keeps item in history
        assert!(db.delete_pinboard(&pb.identifier).unwrap());
        let remaining = db.get_item(&code_item.identifier).unwrap().unwrap();
        assert!(remaining.pinboard_id.is_none());
    }

    #[test]
    fn test_paste_stack_fifo_and_lifo() {
        let db = Database::open_in_memory().unwrap();
        let (a, _) = db
            .ingest_clipboard_payload(NewClipboardPayload {
                plain_text: Some("first".into()),
                ..Default::default()
            })
            .unwrap();
        let (b, _) = db
            .ingest_clipboard_payload(NewClipboardPayload {
                plain_text: Some("second".into()),
                ..Default::default()
            })
            .unwrap();

        let mut stack = PasteStack::new(StackDirection::Fifo);
        stack.push(a.clone());
        stack.push(b.clone());
        assert_eq!(stack.pop_next().unwrap().identifier, a.identifier);
        assert_eq!(stack.pop_next().unwrap().identifier, b.identifier);

        stack.push(a.clone());
        stack.push(b.clone());
        assert_eq!(stack.toggle_direction(), StackDirection::Lifo);
        assert_eq!(stack.pop_next().unwrap().identifier, b.identifier);
        assert_eq!(stack.pop_next().unwrap().identifier, a.identifier);
    }

    #[test]
    fn test_image_background_removal() {
        let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
        for x in 3..7 {
            for y in 3..7 {
                img.put_pixel(x, y, Rgba([20, 40, 200, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();

        let cleaned = remove_solid_background(&buf.into_inner(), 15).unwrap();
        let decoded = image::load_from_memory(&cleaned).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0[3], 0);
        assert_eq!(decoded.get_pixel(5, 5).0[3], 255);
    }
}
