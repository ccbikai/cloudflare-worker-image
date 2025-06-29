use crate::error::AppError;
use image::{
    codecs::{avif::AvifEncoder, jpeg::JpegEncoder, png::PngEncoder, webp::WebPEncoder},
    ColorType, DynamicImage, ImageFormat,
};

pub fn encode_image(
    dyn_image: DynamicImage,
    format: ImageFormat,
    quality: Option<u8>,
) -> Result<Vec<u8>, AppError> {
    let mut buf = Vec::new();
    let format_str = format!("{:?}", format).to_lowercase();

    match format {
        ImageFormat::Jpeg => {
            let q = quality.unwrap_or(95);
            let mut encoder = JpegEncoder::new_with_quality(&mut buf, q);
            let rgb_image = dyn_image.to_rgb8();
            encoder
                .encode(
                    &rgb_image,
                    dyn_image.width(),
                    dyn_image.height(),
                    ColorType::Rgb8.into(),
                )
                .map_err(|e| AppError::ImageEncodeError { format: format_str, source: e })?;
        }
        ImageFormat::Png => {
            let compression = match quality {
                Some(q) if q <= 9 => image::codecs::png::CompressionType::Best,
                _ => image::codecs::png::CompressionType::Default,
            };
            let encoder = PngEncoder::new_with_quality(
                &mut buf,
                compression,
                image::codecs::png::FilterType::Sub,
            );
            dyn_image.write_with_encoder(encoder).map_err(|e| AppError::ImageEncodeError { format: format_str, source: e })?;
        }
        ImageFormat::WebP => {
            let encoder = WebPEncoder::new_lossless(&mut buf);
            dyn_image.write_with_encoder(encoder).map_err(|e| AppError::ImageEncodeError { format: format_str, source: e })?;
        }
        ImageFormat::Avif => {
            let q = quality.unwrap_or(95);
            let speed = 8; // speed: 1 (slowest) to 10 (fastest)
            let encoder = AvifEncoder::new_with_speed_quality(&mut buf, speed, q);
            dyn_image.write_with_encoder(encoder).map_err(|e| AppError::ImageEncodeError { format: format_str, source: e })?;
        }
        _ => {
            dyn_image
                .write_to(&mut std::io::Cursor::new(&mut buf), format)
                .map_err(|e| AppError::ImageEncodeError { format: format_str, source: e })?;
        }
    }

    Ok(buf)
}

pub fn get_image_output_format(format: &str) -> (ImageFormat, &'static str) {
    match format.to_lowercase().as_str() {
        "png" => (ImageFormat::Png, "image/png"),
        "jpeg" | "jpg" => (ImageFormat::Jpeg, "image/jpeg"),
        "webp" => (ImageFormat::WebP, "image/webp"),
        "bmp" => (ImageFormat::Bmp, "image/bmp"),
        "ico" => (ImageFormat::Ico, "image/x-icon"),
        "tiff" => (ImageFormat::Tiff, "image/tiff"),
        "avif" => (ImageFormat::Avif, "image/avif"),
        "gif" => (ImageFormat::Gif, "image/gif"),
        _ => (ImageFormat::Png, "image/png"), // Default to PNG
    }
}
