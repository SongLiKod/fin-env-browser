use crate::models::{PluginInfo, Settings};

pub const VIDEO_SNIFFER: &str = "video-sniffer";

pub fn list(settings: &Settings) -> Vec<PluginInfo> {
    vec![PluginInfo {
        id: VIDEO_SNIFFER.into(),
        name: "视频嗅探".into(),
        description: "监听当前受控标签页的媒体请求，汇总可下载的视频/音频地址。".into(),
        version: "1.0.0".into(),
        enabled: is_enabled(settings, VIDEO_SNIFFER),
        built_in: true,
    }]
}

pub fn is_valid(id: &str) -> bool {
    id == VIDEO_SNIFFER
}

pub fn is_enabled(settings: &Settings, id: &str) -> bool {
    settings.enabled_plugins.iter().any(|item| item == id)
}
