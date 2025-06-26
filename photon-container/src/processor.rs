use actix_web::{error, web};
use image::{
    codecs::jpeg::JpegEncoder, ColorType, GenericImageView, ImageBuffer, ImageFormat, Rgba,
};
use photon_rs::{
    channels, conv, effects, filters, monochrome, multiple, transform, PhotonImage,
};
use serde::Deserialize;
use std::io::Cursor;

#[derive(Deserialize, Debug)]
pub struct ImageQuery {
    pub url: String,
    pub action: String,
    pub format: Option<String>,
    pub quality: Option<u8>,
}

// Helper to parse parameters with a default value
fn parse_param<T: std::str::FromStr>(s: Option<&&str>, default: T) -> T {
    s.and_then(|p| p.parse::<T>().ok()).unwrap_or(default)
}

fn get_image_output_format(format: &str) -> (ImageFormat, &'static str) {
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

pub async fn process_image(
    query: &web::Query<ImageQuery>,
) -> Result<(Vec<u8>, &'static str), error::Error> {
    println!(
        "[LOG] url: {} Received new image processing request: {:?}",
        query.url, query
    );

    // 1. Fetch the image from the URL
    let res = reqwest::get(&query.url).await.map_err(|e| {
        eprintln!("[ERROR] Failed to fetch image URL {}: {}", query.url, e);
        error::ErrorInternalServerError(format!(
            "Failed to fetch image from URL: {} ({})",
            query.url, e
        ))
    })?;

    if !res.status().is_success() {
        let status = res.status();
        let actix_status = actix_web::http::StatusCode::from_u16(status.as_u16())
            .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
        eprintln!(
            "[ERROR] Failed to fetch image URL {}: Status {}",
            query.url, status
        );
        return Err(error::InternalError::new(
            format!(
                "Upstream image fetch failed for url: {} (status: {})",
                query.url, status
            ),
            actix_status,
        )
        .into());
    }

    let image_bytes = res.bytes().await.map_err(|e| {
        eprintln!("[ERROR] Failed to read image bytes from {}: {}", query.url, e);
        error::ErrorInternalServerError(format!(
            "Failed to read image bytes from {}: {}",
            query.url, e
        ))
    })?;

    // 2. Open the image using photon
    let dyn_image = image::load_from_memory(&image_bytes).map_err(|e| {
        eprintln!("[ERROR] Failed to decode image from {}: {}", query.url, e);
        error::ErrorInternalServerError(format!("Failed to decode image from {}: {}", query.url, e))
    })?;
    let (width, height) = dyn_image.dimensions();
    let raw_pixels = dyn_image.to_rgba8().into_raw();
    let mut img = PhotonImage::new(raw_pixels, width, height);

    // 3. Apply effects from the action string
    let actions = query.action.split('|').filter(|s| !s.is_empty());

    for action_str in actions {
        let parts: Vec<&str> = action_str.split('!').collect();
        let action_name = parts[0];
        let options = if parts.len() > 1 { parts[1] } else { "" };
        let params: Vec<&str> = options.split(',').filter(|s| !s.is_empty()).collect();

        println!(
            "[LOG] url: {} Applying action: {} with params {:?}",
            query.url, action_name, params
        );

        match action_name {
            // --- transform ---
            "resize" => {
                if params.len() >= 2 {
                    let width = parse_param(params.get(0), 0);
                    let height = parse_param(params.get(1), 0);
                    let filter = if params.len() > 2 {
                        match params[2] {
                            "1" | "nearest" => transform::SamplingFilter::Nearest,
                            "2" | "triangle" => transform::SamplingFilter::Triangle,
                            "3" | "catmullrom" => transform::SamplingFilter::CatmullRom,
                            "4" | "gaussian" => transform::SamplingFilter::Gaussian,
                            _ => transform::SamplingFilter::Lanczos3,
                        }
                    } else {
                        transform::SamplingFilter::Lanczos3
                    };
                    img = transform::resize(&img, width, height, filter);
                }
            }
            "crop" => {
                if params.len() >= 4 {
                    let x1 = parse_param(params.get(0), 0);
                    let y1 = parse_param(params.get(1), 0);
                    let x2 = parse_param(params.get(2), img.get_width());
                    let y2 = parse_param(params.get(3), img.get_height());
                    img = transform::crop(&img, x1, y1, x2, y2);
                }
            }
            "fliph" => transform::fliph(&mut img),
            "flipv" => transform::flipv(&mut img),

            // --- multiple ---
            "watermark" => {
                if !params.is_empty() {
                    let watermark_url = params[0];
                    let x = parse_param(params.get(1), 0);
                    let y = parse_param(params.get(2), 0);

                    let wm_res = reqwest::get(watermark_url).await.map_err(|e| {
                        error::ErrorInternalServerError(format!(
                            "url: {} failed to fetch watermark image {}: {}",
                            query.url, watermark_url, e
                        ))
                    })?;
                    let wm_bytes = wm_res.bytes().await.map_err(|e| {
                        error::ErrorInternalServerError(format!(
                            "url: {} failed to read watermark image bytes: {}",
                            query.url, e
                        ))
                    })?;
                    let watermark_dyn_img = image::load_from_memory(&wm_bytes).map_err(|e| {
                        error::ErrorInternalServerError(format!(
                            "url: {} failed to decode watermark image: {}",
                            query.url, e
                        ))
                    })?;
                    let (width, height) = watermark_dyn_img.dimensions();
                    let raw_pixels = watermark_dyn_img.to_rgba8().into_raw();
                    let watermark_img = PhotonImage::new(raw_pixels, width, height);

                    multiple::watermark(&mut img, &watermark_img, x, y);
                }
            }
            "blend" => {
                if params.len() >= 2 {
                    let blend_url = params[0];
                    let blend_mode = params[1];
                    let wm_res = reqwest::get(blend_url).await.map_err(|e| {
                        error::ErrorInternalServerError(format!(
                            "url: {} failed to fetch blend image {}: {}",
                            query.url, blend_url, e
                        ))
                    })?;
                    let wm_bytes = wm_res.bytes().await.map_err(|e| {
                        error::ErrorInternalServerError(format!(
                            "url: {} failed to read blend image bytes: {}",
                            query.url, e
                        ))
                    })?;
                    let top_dyn_img = image::load_from_memory(&wm_bytes).map_err(|e| {
                        error::ErrorInternalServerError(format!(
                            "url: {} failed to decode blend image: {}",
                            query.url, e
                        ))
                    })?;
                    let (width, height) = top_dyn_img.dimensions();
                    let raw_pixels = top_dyn_img.to_rgba8().into_raw();
                    let top_img = PhotonImage::new(raw_pixels, width, height);

                    multiple::blend(&mut img, &top_img, blend_mode);
                }
            }

            // --- effects ---
            "solarize" => effects::solarize(&mut img),
            "colorize" => effects::colorize(&mut img),
            "frosted_glass" => effects::frosted_glass(&mut img),
            "inc_brightness" => {
                let val = parse_param(params.get(0), 10u8);
                effects::inc_brightness(&mut img, val);
            }
            "adjust_contrast" => {
                let val = parse_param(params.get(0), 0.1f32);
                effects::adjust_contrast(&mut img, val);
            }
            "tint" => {
                if params.len() >= 3 {
                    let r = parse_param(params.get(0), 0u8);
                    let g = parse_param(params.get(1), 0u8);
                    let b = parse_param(params.get(2), 0u8);
                    effects::tint(&mut img, r.into(), g.into(), b.into());
                }
            }

            // --- filters ---
            "dramatic" => filters::dramatic(&mut img),
            "lofi" => filters::lofi(&mut img),
            // "california", "oceanic", "vintage" etc. are not available in this version of photon-rs
            // --- monochrome ---
            "grayscale" => monochrome::grayscale(&mut img),
            "sepia" => monochrome::sepia(&mut img),

            // --- channels ---
            "alter_channel" => {
                if params.len() >= 2 {
                    let channel = parse_param(params.get(0), 0u8);
                    let amount = parse_param(params.get(1), 10i16);
                    if channel <= 2 {
                        channels::alter_channel(&mut img, channel as usize, amount);
                    }
                }
            }
            "swap_channels" => {
                if params.len() >= 2 {
                    let channel1 = parse_param(params.get(0), 0u8);
                    let channel2 = parse_param(params.get(1), 1u8);
                    if channel1 <= 2 && channel2 <= 2 {
                        channels::swap_channels(&mut img, channel1 as usize, channel2 as usize);
                    }
                }
            }
            "remove_red_channel" => {
                let min_filter = parse_param(params.get(0), 255u8);
                channels::remove_red_channel(&mut img, min_filter);
            }
            "remove_green_channel" => {
                let min_filter = parse_param(params.get(0), 255u8);
                channels::remove_green_channel(&mut img, min_filter);
            }
            "remove_blue_channel" => {
                let min_filter = parse_param(params.get(0), 255u8);
                channels::remove_blue_channel(&mut img, min_filter);
            }

            // --- conv ---
            "sharpen" => conv::sharpen(&mut img),
            "box_blur" => conv::box_blur(&mut img),
            "edge_detection" => conv::edge_detection(&mut img),
            "emboss" => conv::emboss(&mut img),

            _ => {
                println!("[WARN] url: {} Unknown action: {}", query.url, action_name);
            }
        }
    }

    // 4. Encode the image to the desired format
    let format_str = query.format.as_deref().unwrap_or("png");
    let (output_format, content_type) = get_image_output_format(format_str);

    let raw_pixels = img.get_raw_pixels();
    let mut buf = Vec::new();

    if matches!(output_format, ImageFormat::Jpeg) {
        let quality = query.quality.unwrap_or(95);
        let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
        encoder
            .encode(
                &raw_pixels,
                img.get_width(),
                img.get_height(),
                ColorType::Rgba8.into(),
            )
            .map_err(|e| {
                eprintln!("[ERROR] url: {} Failed to encode JPEG: {}", query.url, e);
                error::ErrorInternalServerError(format!(
                    "Failed to encode JPEG for url {}: {}",
                    query.url, e
                ))
            })?;
    } else {
        let image_buffer =
            ImageBuffer::<Rgba<u8>, _>::from_raw(img.get_width(), img.get_height(), raw_pixels)
                .ok_or_else(|| {
                    error::ErrorInternalServerError(format!(
                        "Failed to create image buffer for url {}",
                        query.url
                    ))
                })?;
        let dyn_image = image::DynamicImage::ImageRgba8(image_buffer);
        dyn_image
            .write_to(&mut Cursor::new(&mut buf), output_format)
            .map_err(|e| {
                eprintln!(
                    "[ERROR] url: {} Failed to encode image to {}: {}",
                    query.url, format_str, e
                );
                error::ErrorInternalServerError(format!(
                    "Failed to encode image {} to {}: {}",
                    query.url, format_str, e
                ))
            })?;
    }

    Ok((buf, content_type))
}
