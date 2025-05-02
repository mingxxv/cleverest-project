use crate::common::error::{CleverestError, Result};
use crate::common::protocol::{FrameData, PixelFormat};
use image::{DynamicImage, ImageBuffer, Rgba};
use std::time::{SystemTime, UNIX_EPOCH};

// Get current timestamp in milliseconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

// Convert raw image data to a DynamicImage
pub fn bytes_to_image(frame: &FrameData) -> Result<DynamicImage> {
    match frame.format {
        PixelFormat::RGBA => {
            let img_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
                frame.width,
                frame.height,
                frame.data.clone(),
            )
            .ok_or_else(|| CleverestError::Unknown("Failed to create image from raw data".into()))?;
            Ok(DynamicImage::ImageRgba8(img_buffer))
        }
        _ => Err(CleverestError::Unknown(
            format!("Unsupported pixel format: {:?}", frame.format)
        )),
    }
}

// Convert a DynamicImage to raw bytes
pub fn image_to_bytes(img: &DynamicImage) -> Result<(Vec<u8>, u32, u32, PixelFormat)> {
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let format = PixelFormat::RGBA;
    
    // Convert to raw bytes
    let raw_data = rgba.into_raw();
    
    Ok((raw_data, width, height, format))
}