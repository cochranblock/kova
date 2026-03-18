//! Multi-provider image generation client.
//! t122=T122, t123=T123, t124=T124, t125=T125.
//! f189=f189, f190=f190, f191=f191, f192=f192, f193=f193.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::Path;
use std::time::Instant;

// ── Types ──────────────────────────────────────────────────

/// t122: Image generation provider.
pub enum T122 {
    StableDiffusion { url: String },
    DallE { api_key: String },
    Local { model_path: String },
}

/// t123: Image generation request.
pub struct T123 {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub steps: Option<u32>,
}

impl Default for T123 {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: None,
        }
    }
}

/// t124: Image generation result.
pub struct T124 {
    pub image_bytes: Vec<u8>,
    pub format: T125,
    pub provider: String,
    pub generation_ms: u64,
}

/// t125: Output image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T125 {
    Png,
    Jpeg,
    Webp,
}

impl T125 {
    /// File extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }
}

// ── Functions ──────────────────────────────────────────────

/// f189: Dispatch image generation to the right provider.
pub fn f189(provider: &T122, request: &T123) -> Result<T124, String> {
    match provider {
        T122::StableDiffusion { url } => f190(url, request),
        T122::DallE { api_key } => f191(api_key, request),
        T122::Local { model_path } => {
            Err(format!("local provider not yet implemented: {}", model_path))
        }
    }
}

/// f190: Call Stable Diffusion WebUI API (POST /sdapi/v1/txt2img).
/// Response JSON: `{ "images": ["base64..."] }`.
pub fn f190(url: &str, request: &T123) -> Result<T124, String> {
    use reqwest::blocking::Client;
    use serde_json::{json, Value};

    let endpoint = format!("{}/sdapi/v1/txt2img", url.trim_end_matches('/'));
    let client = Client::new();

    let mut body = json!({
        "prompt": request.prompt,
        "width": request.width,
        "height": request.height,
    });

    if let Some(ref neg) = request.negative_prompt {
        body["negative_prompt"] = json!(neg);
    }
    if let Some(steps) = request.steps {
        body["steps"] = json!(steps);
    }

    let start = Instant::now();
    let resp = client
        .post(&endpoint)
        .json(&body)
        .send()
        .map_err(|e| format!("SD request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("SD API returned {}", resp.status()));
    }

    let json: Value = resp
        .json()
        .map_err(|e| format!("SD response parse failed: {}", e))?;

    let b64 = json["images"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .ok_or_else(|| "SD response missing images array".to_string())?;

    let image_bytes = base64_decode(b64)?;
    let generation_ms = start.elapsed().as_millis() as u64;

    Ok(T124 {
        image_bytes,
        format: T125::Png,
        provider: "stable-diffusion".into(),
        generation_ms,
    })
}

/// f191: Call OpenAI DALL-E API (POST /v1/images/generations).
/// Response JSON: `{ "data": [{"url": "..."}] }`.
pub fn f191(api_key: &str, request: &T123) -> Result<T124, String> {
    use reqwest::blocking::Client;
    use serde_json::{json, Value};

    let client = Client::new();

    // DALL-E accepts specific sizes; pick closest.
    let size = dalle_size(request.width, request.height);

    let body = json!({
        "model": "dall-e-3",
        "prompt": request.prompt,
        "n": 1,
        "size": size,
        "response_format": "url",
    });

    let start = Instant::now();
    let resp = client
        .post("https://api.openai.com/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .map_err(|e| format!("DALL-E request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("DALL-E API returned {}: {}", status, text));
    }

    let json: Value = resp
        .json()
        .map_err(|e| format!("DALL-E response parse failed: {}", e))?;

    let image_url = json["data"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["url"].as_str())
        .ok_or_else(|| "DALL-E response missing data[0].url".to_string())?;

    // Download the image from the returned URL.
    let img_resp = client
        .get(image_url)
        .send()
        .map_err(|e| format!("image download failed: {}", e))?;

    let image_bytes = img_resp
        .bytes()
        .map_err(|e| format!("image read failed: {}", e))?
        .to_vec();

    let generation_ms = start.elapsed().as_millis() as u64;

    Ok(T124 {
        image_bytes,
        format: T125::Png,
        provider: "dall-e".into(),
        generation_ms,
    })
}

/// f192: Write image bytes to disk.
pub fn f192(result: &T124, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &result.image_bytes)?;
    Ok(())
}

/// f193: List available Stable Diffusion models.
pub fn f193(url: &str) -> Result<Vec<String>, String> {
    use reqwest::blocking::Client;
    use serde_json::Value;

    let endpoint = format!("{}/sdapi/v1/sd-models", url.trim_end_matches('/'));
    let client = Client::new();

    let resp = client
        .get(&endpoint)
        .send()
        .map_err(|e| format!("SD models request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("SD models API returned {}", resp.status()));
    }

    let json: Value = resp
        .json()
        .map_err(|e| format!("SD models response parse failed: {}", e))?;

    let models = json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v["title"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}

// ── Helpers ────────────────────────────────────────────────

/// Decode base64 string to bytes. Handles optional data-URI prefix.
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Strip data URI prefix if present (e.g. "data:image/png;base64,").
    let raw = if let Some(pos) = input.find(",") {
        &input[pos + 1..]
    } else {
        input
    };

    // Simple base64 decoder — no extra deps.
    b64_decode_bytes(raw)
}

/// Minimal base64 decode (RFC 4648, standard alphabet, with padding).
fn b64_decode_bytes(s: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 char: {}", c as char)),
        }
    }

    let input: Vec<u8> = s.bytes().filter(|&b| b != b'=' && b != b'\n' && b != b'\r' && b != b' ').collect();
    let mut out = Vec::with_capacity(input.len() * 3 / 4);

    let chunks = input.chunks(4);
    for chunk in chunks {
        let len = chunk.len();
        if len < 2 {
            if len == 1 {
                return Err("invalid base64: trailing single byte".into());
            }
            break;
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if len > 2 {
            let c = val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if len > 3 {
                let d = val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }

    // Verify against TABLE to silence unused warning.
    debug_assert!(TABLE[0] == b'A');

    Ok(out)
}

/// Map requested dimensions to a DALL-E supported size string.
fn dalle_size(w: u32, h: u32) -> &'static str {
    if w <= 256 && h <= 256 {
        "256x256"
    } else if w <= 512 && h <= 512 {
        "512x512"
    } else if w > h {
        "1792x1024"
    } else if h > w {
        "1024x1792"
    } else {
        "1024x1024"
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::kova_test;

    kova_test!(f189, image_request_defaults, {
        let req = T123::default();
        assert_eq!(req.width, 512);
        assert_eq!(req.height, 512);
        assert!(req.prompt.is_empty());
        assert!(req.negative_prompt.is_none());
        assert!(req.steps.is_none());
    });

    kova_test!(f189, image_format_extensions, {
        assert_eq!(T125::Png.extension(), "png");
        assert_eq!(T125::Jpeg.extension(), "jpg");
        assert_eq!(T125::Webp.extension(), "webp");
    });

    kova_test!(f192, save_image_to_temp_file, {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("output.png");
        let result = T124 {
            image_bytes: vec![0x89, 0x50, 0x4E, 0x47], // PNG magic bytes
            format: T125::Png,
            provider: "test".into(),
            generation_ms: 42,
        };
        f192(&result, &path).unwrap();
        let read_back = std::fs::read(&path).unwrap();
        assert_eq!(read_back, vec![0x89, 0x50, 0x4E, 0x47]);
    });

    kova_test!(f192, save_image_creates_parent_dirs, {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("deep").join("nested").join("image.jpg");
        let result = T124 {
            image_bytes: vec![0xFF, 0xD8, 0xFF], // JPEG magic
            format: T125::Jpeg,
            provider: "test".into(),
            generation_ms: 0,
        };
        f192(&result, &path).unwrap();
        assert!(path.exists());
    });

    kova_test!(f189, base64_decode_roundtrip, {
        // "Hello" in base64 = "SGVsbG8="
        let decoded = base64_decode("SGVsbG8=").unwrap();
        assert_eq!(decoded, b"Hello");
    });

    kova_test!(f189, base64_decode_with_data_uri, {
        let decoded = base64_decode("data:image/png;base64,SGVsbG8=").unwrap();
        assert_eq!(decoded, b"Hello");
    });

    kova_test!(f189, dalle_size_mapping, {
        assert_eq!(dalle_size(256, 256), "256x256");
        assert_eq!(dalle_size(512, 512), "512x512");
        assert_eq!(dalle_size(1024, 1024), "1024x1024");
        assert_eq!(dalle_size(1920, 1080), "1792x1024");
        assert_eq!(dalle_size(1080, 1920), "1024x1792");
    });
}