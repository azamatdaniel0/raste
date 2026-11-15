use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::models::{ItemPreview, ItemType, NewClipboardPayload};
use crate::processor::{create_thumbnail_and_dimensions, is_secret_or_confidential};

/// Parses a single color string (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb(...)`, `rgba(...)`, `hsl(...)`).
/// Returns `(canonical_hex, [r, g, b, a], (h, s, l))`.
pub fn parse_color(text: &str) -> Option<(String, [u8; 4], (f32, f32, f32))> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 64 || trimmed.contains('\n') {
        return None;
    }

    if let Some(hex) = trimmed.strip_prefix('#') {
        let rgba = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                [r, g, b, 255]
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                [r, g, b, 255]
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                [r, g, b, a]
            }
            _ => return None,
        };
        let canonical = format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2]);
        return Some((canonical, rgba, rgb_to_hsl(rgba[0], rgba[1], rgba[2])));
    }

    static RGB_RE: OnceLock<Regex> = OnceLock::new();
    let rgb_re = RGB_RE.get_or_init(|| {
        Regex::new(
            r"(?i)^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*([0-9]*\.?[0-9]+))?\s*\)$",
        )
        .unwrap()
    });
    if let Some(caps) = rgb_re.captures(trimmed) {
        let r: u8 = caps.get(1)?.as_str().parse::<u16>().ok()?.min(255) as u8;
        let g: u8 = caps.get(2)?.as_str().parse::<u16>().ok()?.min(255) as u8;
        let b: u8 = caps.get(3)?.as_str().parse::<u16>().ok()?.min(255) as u8;
        let a = if let Some(a_str) = caps.get(4) {
            let af: f32 = a_str.as_str().parse().ok()?;
            if af <= 1.0 {
                (af.clamp(0.0, 1.0) * 255.0).round() as u8
            } else {
                af.clamp(0.0, 255.0).round() as u8
            }
        } else {
            255
        };
        let canonical = format!("#{:02X}{:02X}{:02X}", r, g, b);
        return Some((canonical, [r, g, b, a], rgb_to_hsl(r, g, b)));
    }

    static HSL_RE: OnceLock<Regex> = OnceLock::new();
    let hsl_re = HSL_RE.get_or_init(|| {
        Regex::new(r"(?i)^hsla?\(\s*(\d{1,3}(?:\.\d+)?)\s*,\s*(\d{1,3}(?:\.\d+)?)%\s*,\s*(\d{1,3}(?:\.\d+)?)%(?:\s*,\s*([0-9]*\.?[0-9]+))?\s*\)$").unwrap()
    });
    if let Some(caps) = hsl_re.captures(trimmed) {
        let h: f32 = caps.get(1)?.as_str().parse::<f32>().ok()? % 360.0;
        let s: f32 = caps.get(2)?.as_str().parse::<f32>().ok()?.clamp(0.0, 100.0);
        let l: f32 = caps.get(3)?.as_str().parse::<f32>().ok()?.clamp(0.0, 100.0);
        let a = if let Some(a_str) = caps.get(4) {
            let af: f32 = a_str.as_str().parse().ok()?;
            (af.clamp(0.0, 1.0) * 255.0).round() as u8
        } else {
            255
        };
        let (r, g, b) = hsl_to_rgb(h, s, l);
        let canonical = format!("#{:02X}{:02X}{:02X}", r, g, b);
        return Some((canonical, [r, g, b, a], (h, s, l)));
    }

    None
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, (l * 100.0).round());
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let mut h = if (max - rf).abs() < f32::EPSILON {
        (gf - bf) / d + if gf < bf { 6.0 } else { 0.0 }
    } else if (max - gf).abs() < f32::EPSILON {
        (bf - rf) / d + 2.0
    } else {
        (rf - gf) / d + 4.0
    };
    h /= 6.0;
    ((h * 360.0).round(), (s * 100.0).round(), (l * 100.0).round())
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let s = s / 100.0;
    let l = l / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// Parses single-token HTTP/HTTPS URLs and returns `(full_url, domain_host, suggested_title)`.
pub fn parse_url(text: &str) -> Option<(String, String, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return None;
    }

    let rest = if let Some(r) = trimmed.strip_prefix("https://") {
        r
    } else if let Some(r) = trimmed.strip_prefix("http://") {
        r
    } else {
        return None;
    };

    if rest.is_empty() || !rest.contains('.') && !rest.starts_with("localhost") {
        return None;
    }

    let host_port = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host_port.split(':').next().unwrap_or(host_port);
    if host.is_empty() {
        return None;
    }

    let clean_domain = host.strip_prefix("www.").unwrap_or(host).to_string();
    let path_hint = rest
        .split(['/', '?', '#'])
        .skip(1)
        .filter(|seg| !seg.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" / ");

    let title = if path_hint.is_empty() {
        clean_domain.clone()
    } else {
        format!("{clean_domain} — {path_hint}")
    };

    Some((trimmed.to_string(), clean_domain, title))
}

/// Lightweight code language detection for monospace formatting & badge hints.
pub fn detect_code_language(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.len() < 4 {
        return None;
    }

    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return Some("json".to_string());
        }
    }

    if trimmed.contains("fn ") && (trimmed.contains("->") || trimmed.contains("let mut ") || trimmed.contains("impl ")) {
        return Some("rust".to_string());
    }
    if trimmed.contains("def ") && trimmed.contains(':') && (trimmed.contains("import ") || trimmed.contains("return ")) {
        return Some("python".to_string());
    }
    if (trimmed.contains("const ") || trimmed.contains("function ") || trimmed.contains("=>"))
        && (trimmed.contains('{') || trimmed.contains(';'))
    {
        return Some("javascript".to_string());
    }
    if trimmed.starts_with('<') && trimmed.contains("</") && trimmed.ends_with('>') {
        return Some("html".to_string());
    }

    let upper = trimmed.to_ascii_uppercase();
    if (upper.starts_with("SELECT ") || upper.starts_with("INSERT ") || upper.starts_with("CREATE TABLE "))
        && upper.contains("FROM ")
    {
        return Some("sql".to_string());
    }

    if trimmed.starts_with("$ ")
        || trimmed.starts_with("cargo ")
        || trimmed.starts_with("git ")
        || trimmed.starts_with("npm ")
        || trimmed.starts_with("docker ")
        || trimmed.starts_with("curl ")
        || trimmed.starts_with("#!/")
    {
        return Some("shell".to_string());
    }

    None
}

/// Classifies a clipboard payload into `ItemType`, generates a clean title,
/// builds `ItemPreview`, and determines `is_confidential`.
pub fn classify_and_build_preview(
    payload: &NewClipboardPayload,
    detect_secrets: bool,
) -> (ItemType, String, ItemPreview, bool) {
    if !payload.file_paths.is_empty() {
        let file_names: Vec<String> = payload
            .file_paths
            .iter()
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p)
                    .to_string()
            })
            .collect();
        let file_count = file_names.len();
        let title = payload.custom_title.clone().unwrap_or_else(|| {
            if file_count == 1 {
                file_names[0].clone()
            } else {
                format!("{file_count} Files")
            }
        });
        let snippet = file_names.iter().take(5).cloned().collect::<Vec<_>>().join("\n");
        let preview = ItemPreview {
            snippet,
            file_names,
            file_count,
            ..Default::default()
        };
        return (ItemType::Files, title, preview, false);
    }

    if let Some(ref png_bytes) = payload.image_png {
        let (width, height, thumb_b64) =
            create_thumbnail_and_dimensions(png_bytes, 280).unwrap_or((0, 0, String::new()));
        let title = payload
            .custom_title
            .clone()
            .unwrap_or_else(|| format!("Image {width}×{height}"));
        let preview = ItemPreview {
            snippet: format!("PNG Image ({width} × {height})"),
            image_width: Some(width),
            image_height: Some(height),
            image_format: Some("PNG".to_string()),
            image_size_bytes: Some(png_bytes.len()),
            thumbnail_png_b64: if thumb_b64.is_empty() {
                None
            } else {
                Some(thumb_b64)
            },
            ..Default::default()
        };
        return (ItemType::Image, title, preview, false);
    }

    if let Some(ref text) = payload.plain_text {
        let trimmed = text.trim();
        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();
        let line_count = text.lines().count().max(1);
        let snippet: String = trimmed.chars().take(600).collect();

        if let Some((hex_color, rgba, hsl)) = parse_color(trimmed) {
            let title = payload
                .custom_title
                .clone()
                .unwrap_or_else(|| hex_color.clone());
            let preview = ItemPreview {
                snippet: hex_color.clone(),
                char_count,
                word_count,
                line_count,
                color_hex: Some(hex_color),
                color_rgba: Some(rgba),
                color_hsl: Some(hsl),
                ..Default::default()
            };
            return (ItemType::Color, title, preview, false);
        }

        if let Some((full_url, domain, url_title)) = parse_url(trimmed) {
            let title = payload.custom_title.clone().unwrap_or(url_title);
            let preview = ItemPreview {
                snippet: full_url.clone(),
                char_count,
                word_count,
                line_count,
                link_url: Some(full_url),
                link_domain: Some(domain),
                ..Default::default()
            };
            return (ItemType::Link, title, preview, false);
        }

        let is_confidential = detect_secrets && is_secret_or_confidential(text);
        let language_hint = if is_confidential {
            None
        } else {
            detect_code_language(text)
        };
        let title = payload.custom_title.clone().unwrap_or_else(|| {
            let first_line = trimmed.lines().next().unwrap_or("Text Snippet").trim();
            let truncated: String = first_line.chars().take(72).collect();
            if truncated.is_empty() {
                "Text Snippet".to_string()
            } else {
                truncated
            }
        });

        let preview = ItemPreview {
            snippet,
            char_count,
            word_count,
            line_count,
            language_hint,
            ..Default::default()
        };
        return (ItemType::Text, title, preview, is_confidential);
    }

    let title = payload
        .custom_title
        .clone()
        .unwrap_or_else(|| "Empty Item".to_string());
    (ItemType::Unknown, title, ItemPreview::default(), false)
}
