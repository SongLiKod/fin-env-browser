use std::collections::HashMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::{now_iso, SniffedVideo};

/// A bounded, in-memory index of media responses observed in controlled tabs.
/// It intentionally keeps request headers out of the persisted/UI model.
#[derive(Default)]
pub struct VideoStore {
    entries: HashMap<String, Vec<SniffedVideo>>,
    pending: HashMap<String, PendingRequest>,
}

struct PendingRequest {
    env_id: String,
    video_id: Option<String>,
    referer: String,
    user_agent: String,
}

impl VideoStore {
    pub fn list(&self, env_id: &str) -> Vec<SniffedVideo> {
        self.entries.get(env_id).cloned().unwrap_or_default()
    }

    pub fn clear(&mut self, env_id: &str) {
        self.entries.remove(env_id);
        self.pending.retain(|_, request| request.env_id != env_id);
    }

    pub fn get(&self, env_id: &str, video_id: &str) -> Option<SniffedVideo> {
        self.entries
            .get(env_id)
            .and_then(|items| items.iter().find(|item| item.id == video_id))
            .cloned()
    }

    pub fn record_request(&mut self, env_id: &str, params: &Value) {
        let Some(request_id) = params.get("requestId").and_then(Value::as_str) else {
            return;
        };
        let Some(request) = params.get("request") else {
            return;
        };
        let Some(url) = request.get("url").and_then(Value::as_str) else {
            return;
        };
        let request_type = request
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !is_media_candidate(url, "", request_type) {
            return;
        }
        let headers = request.get("headers").and_then(Value::as_object);
        self.pending.insert(
            pending_key(env_id, request_id),
            PendingRequest {
                env_id: env_id.to_string(),
                video_id: None,
                referer: header(headers, "referer"),
                user_agent: header(headers, "user-agent"),
            },
        );
    }

    pub fn record_response(
        &mut self,
        env_id: &str,
        page_url: &str,
        params: &Value,
    ) -> Option<SniffedVideo> {
        let request_id = params.get("requestId").and_then(Value::as_str)?;
        let response = params.get("response")?;
        let url = response.get("url").and_then(Value::as_str)?;
        let mime_type = response
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let headers = response.get("headers").and_then(Value::as_object);
        let content_type = header(headers, "content-type");
        let mime = if content_type.is_empty() {
            mime_type.clone()
        } else {
            content_type.clone()
        };
        let request_type = params
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !is_media_candidate(url, &mime, request_type) {
            return None;
        }

        let key = pending_key(env_id, request_id);
        let pending = self.pending.get(&key);
        let referer = pending.map(|item| item.referer.clone()).unwrap_or_default();
        let user_agent = pending
            .map(|item| item.user_agent.clone())
            .unwrap_or_default();
        let kind = media_kind(url, &mime);
        let id = video_id(url);
        let title = media_title(url, &id);

        let entry = SniffedVideo {
            id: id.clone(),
            env_id: env_id.to_string(),
            url: url.to_string(),
            title,
            mime_type: mime,
            kind: kind.clone(),
            size: None,
            page_url: page_url.to_string(),
            referer,
            user_agent,
            downloadable: kind == "video" || kind == "audio",
            discovered_at: now_iso(),
        };

        let items = self.entries.entry(env_id.to_string()).or_default();
        if let Some(existing) = items.iter_mut().find(|item| item.url == entry.url) {
            existing.mime_type = entry.mime_type.clone();
            existing.kind = entry.kind.clone();
            existing.discovered_at = entry.discovered_at.clone();
            existing.downloadable = entry.downloadable;
            let existing = existing.clone();
            if let Some(pending) = self.pending.get_mut(&key) {
                pending.video_id = Some(existing.id.clone());
            }
            return Some(existing);
        }
        if items.len() >= 300 {
            items.remove(0);
        }
        items.push(entry.clone());
        if let Some(pending) = self.pending.get_mut(&key) {
            pending.video_id = Some(id);
        }
        Some(entry)
    }

    pub fn record_finished(&mut self, env_id: &str, params: &Value) {
        let Some(request_id) = params.get("requestId").and_then(Value::as_str) else {
            return;
        };
        let key = pending_key(env_id, request_id);
        let Some(pending) = self.pending.remove(&key) else {
            return;
        };
        let Some(video_id) = pending.video_id else {
            return;
        };
        let size = params.get("encodedDataLength").and_then(Value::as_u64);
        let Some(size) = size else {
            return;
        };
        if let Some(items) = self.entries.get_mut(env_id) {
            if let Some(item) = items.iter_mut().find(|item| item.id == video_id) {
                item.size = Some(size);
            }
        }
    }
}

fn pending_key(env_id: &str, request_id: &str) -> String {
    format!("{env_id}:{request_id}")
}

fn header(headers: Option<&serde_json::Map<String, Value>>, name: &str) -> String {
    let Some(headers) = headers else {
        return String::new();
    };
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn is_media_candidate(url: &str, mime: &str, request_type: &str) -> bool {
    let mime = mime.to_ascii_lowercase();
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    let extension = path.rsplit('.').next().unwrap_or_default();
    let media_mime = mime.starts_with("video/")
        || mime.starts_with("audio/")
        || mime.contains("mpegurl")
        || mime.contains("dash+xml")
        || mime.contains("octet-stream") && is_media_extension(extension);
    let media_extension = is_media_extension(extension);
    request_type.eq_ignore_ascii_case("media")
        || media_mime
        || media_extension
        || path.contains(".m3u8")
        || path.contains(".mpd")
}

fn is_media_extension(extension: &str) -> bool {
    matches!(
        extension,
        "mp4"
            | "webm"
            | "mov"
            | "m4v"
            | "flv"
            | "mkv"
            | "avi"
            | "ts"
            | "m4s"
            | "mp3"
            | "m4a"
            | "aac"
            | "ogg"
            | "wav"
            | "m3u8"
            | "mpd"
    )
}

fn media_kind(url: &str, mime: &str) -> String {
    let mime = mime.to_ascii_lowercase();
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    if path.ends_with(".m3u8") || mime.contains("mpegurl") {
        "hls".into()
    } else if path.ends_with(".mpd") || mime.contains("dash+xml") {
        "dash".into()
    } else if path.ends_with(".ts") || path.ends_with(".m4s") {
        "hls".into()
    } else if mime.starts_with("audio/") {
        "audio".into()
    } else {
        "video".into()
    }
}

fn media_title(url: &str, id: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or_default();
    if name.is_empty() || name == "/" {
        format!("media-{}.mp4", &id[..8.min(id.len())])
    } else {
        name.to_string()
    }
}

fn video_id(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    let short = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("video-{short}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_direct_video_and_hls_playlists() {
        assert!(is_media_candidate(
            "https://cdn.test/a.mp4?token=1",
            "",
            "Media"
        ));
        assert!(is_media_candidate(
            "https://cdn.test/index.m3u8",
            "application/vnd.apple.mpegurl",
            "Fetch"
        ));
        assert!(!is_media_candidate(
            "https://cdn.test/app.js",
            "text/javascript",
            "Script"
        ));
        assert_eq!(media_kind("https://cdn.test/a.mp4", "video/mp4"), "video");
        assert_eq!(media_kind("https://cdn.test/index.m3u8", ""), "hls");
    }

    #[test]
    fn deduplicates_by_url_and_tracks_response_size() {
        let mut store = VideoStore::default();
        store.record_request(
            "env-1",
            &json!({
                "requestId": "r1",
                "request": {
                    "url": "https://cdn.test/a.mp4",
                    "type": "Media",
                    "headers": { "Referer": "https://site.test/watch", "User-Agent": "Test UA" }
                }
            }),
        );
        let first = store
            .record_response(
                "env-1",
                "https://site.test/watch",
                &json!({
                    "requestId": "r1",
                    "response": { "url": "https://cdn.test/a.mp4", "mimeType": "video/mp4" }
                }),
            )
            .expect("video should be detected");
        assert_eq!(first.kind, "video");
        assert_eq!(first.referer, "https://site.test/watch");
        store.record_finished(
            "env-1",
            &json!({ "requestId": "r1", "encodedDataLength": 1024 }),
        );
        assert_eq!(store.get("env-1", &first.id).unwrap().size, Some(1024));
        let second = store
            .record_response(
                "env-1",
                "https://site.test/watch",
                &json!({
                    "requestId": "r2",
                    "response": { "url": "https://cdn.test/a.mp4", "mimeType": "video/mp4" }
                }),
            )
            .expect("duplicate response should resolve");
        assert_eq!(second.id, first.id);
        assert_eq!(store.list("env-1").len(), 1);
    }
}
