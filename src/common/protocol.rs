use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameData {
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub data: Vec<u8>,
    pub format: PixelFormat,
    pub compressed: bool,   // Whether the data is compressed
    pub key_frame: bool,    // Whether this is a key frame (full image) or delta
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PixelFormat {
    RGB,
    RGBA,
    BGR,
    BGRA,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello {
        client_id: String,
        client_version: String,
    },
    Frame(FrameData),
    Goodbye,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Hello {
        server_id: String,
        server_version: String,
    },
    FrameAck {
        timestamp: u64,
        received_at: u64,
    },
    RequestKeyFrame,
    ConfigureQuality {
        quality: u8, // 0-100
    },
    ConfigureFrameRate {
        fps: u8,
    },
    ToggleCapture {
        enabled: bool,
    },
    Goodbye,
}