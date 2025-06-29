use crate::error::AppError;
use crate::utils::parse_param;
use image::GenericImageView;
use log::{info, warn};
use photon_rs::{
    channels, conv, effects, filters, monochrome, multiple, text, transform, PhotonImage,
};

// Helper function to fetch an image from a URL and convert it to a PhotonImage
async fn fetch_photon_image_from_url(url: &str) -> Result<PhotonImage, AppError> {
    let bytes = reqwest::get(url).await?.bytes().await?;
    let dyn_img = image::load_from_memory(&bytes)?;
    let (width, height) = dyn_img.dimensions();
    let raw_pixels = dyn_img.to_rgba8().into_raw();
    Ok(PhotonImage::new(raw_pixels, width, height))
}

pub async fn apply_actions(
    mut img: PhotonImage,
    action_str: &str,
    original_url: &str,
) -> Result<PhotonImage, AppError> {
    let actions = action_str.split('|').filter(|s| !s.is_empty());

    for action_item in actions {
        let mut parts = action_item.splitn(2, '!');
        let action_name = parts.next().unwrap_or("");
        let params: Vec<&str> = parts
            .next()
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.is_empty())
            .collect();

        info!(
            "Applying action: {} with params {:?} {{ url: {} }}",
            action_name, params, original_url
        );

        img = match action_name {
            // --- transform ---
            "resize" => resize(img, &params)?,
            "crop" => crop(img, &params)?,
            "fliph" => {
                transform::fliph(&mut img);
                img
            }
            "flipv" => {
                transform::flipv(&mut img);
                img
            }
            "rotate" => rotate(img, &params)?,

            // --- multiple ---
            "watermark" => watermark(img, &params).await?,
            "blend" => blend(img, &params).await?,
            "draw_text" => draw_text(img, &params)?,

            // --- effects ---
            "solarize" => {
                effects::solarize(&mut img);
                img
            }
            "colorize" => {
                effects::colorize(&mut img);
                img
            }
            "frosted_glass" => {
                effects::frosted_glass(&mut img);
                img
            }
            "inc_brightness" => inc_brightness(img, &params)?,
            "adjust_contrast" => adjust_contrast(img, &params)?,
            "tint" => tint(img, &params)?,

            // --- filters ---
            "filter" => filter(img, &params)?,
            "dramatic" => {
                filters::dramatic(&mut img);
                img
            }
            "lofi" => {
                filters::lofi(&mut img);
                img
            }

            // --- monochrome ---
            "grayscale" => {
                monochrome::grayscale(&mut img);
                img
            }
            "sepia" => {
                monochrome::sepia(&mut img);
                img
            }

            // --- channels ---
            "alter_channel" => alter_channel(img, &params)?,
            "swap_channels" => swap_channels(img, &params)?,
            "remove_red_channel" => remove_channel(img, &params, 'r')?,
            "remove_green_channel" => remove_channel(img, &params, 'g')?,
            "remove_blue_channel" => remove_channel(img, &params, 'b')?,

            // --- conv ---
            "sharpen" => {
                conv::sharpen(&mut img);
                img
            }
            "box_blur" => {
                conv::box_blur(&mut img);
                img
            }
            "edge_detection" => {
                conv::edge_detection(&mut img);
                img
            }
            "emboss" => {
                conv::emboss(&mut img);
                img
            }

            _ => {
                warn!("Unknown action: {} {{ url: {} }}", action_name, original_url);
                img
            }
        };
    }
    Ok(img)
}

fn resize(img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 2 {
        return Err(AppError::InvalidActionParam(
            "resize requires at least 2 parameters: width, height".to_string(),
        ));
    }
    let width = parse_param(params.get(0), 0);
    let height = parse_param(params.get(1), 0);
    let filter = params.get(2).map_or(transform::SamplingFilter::Lanczos3, |s| {
        match *s {
            "1" | "nearest" => transform::SamplingFilter::Nearest,
            "2" | "triangle" => transform::SamplingFilter::Triangle,
            "3" | "catmullrom" => transform::SamplingFilter::CatmullRom,
            "4" | "gaussian" => transform::SamplingFilter::Gaussian,
            _ => transform::SamplingFilter::Lanczos3,
        }
    });
    Ok(transform::resize(&img, width, height, filter))
}

fn crop(img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 4 {
        return Err(AppError::InvalidActionParam(
            "crop requires 4 parameters: x1, y1, x2, y2".to_string(),
        ));
    }
    let x1 = parse_param(params.get(0), 0);
    let y1 = parse_param(params.get(1), 0);
    let x2 = parse_param(params.get(2), img.get_width());
    let y2 = parse_param(params.get(3), img.get_height());
    Ok(transform::crop(&img, x1, y1, x2, y2))
}

fn rotate(img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.is_empty() {
        return Err(AppError::InvalidActionParam(
            "rotate requires 1 parameter: angle".to_string(),
        ));
    }
    let angle = parse_param(params.get(0), 90i32);
    Ok(transform::rotate(&img, angle as f32))
}

async fn watermark(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.is_empty() {
        return Err(AppError::InvalidActionParam(
            "watermark requires at least 1 parameter: watermark_url".to_string(),
        ));
    }
    let watermark_url = params[0];
    let x = parse_param(params.get(1), 0);
    let y = parse_param(params.get(2), 0);

    let watermark_img = fetch_photon_image_from_url(watermark_url).await?;

    multiple::watermark(&mut img, &watermark_img, x, y);
    Ok(img)
}

async fn blend(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 2 {
        return Err(AppError::InvalidActionParam(
            "blend requires 2 parameters: blend_url, blend_mode".to_string(),
        ));
    }
    let blend_url = params[0];
    let blend_mode = params[1];

    let top_img = fetch_photon_image_from_url(blend_url).await?;

    multiple::blend(&mut img, &top_img, blend_mode);
    Ok(img)
}

fn draw_text(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 3 {
        return Err(AppError::InvalidActionParam(
            "draw_text requires at least 3 parameters: text, x, y".to_string(),
        ));
    }
    let text_content = params[0];
    let x = parse_param(params.get(1), 0i32);
    let y = parse_param(params.get(2), 0i32);
    let font_size = parse_param(params.get(3), 24.0f32);
    text::draw_text(&mut img, text_content, x, y, font_size);
    Ok(img)
}

// --- Add missing action functions ---

fn inc_brightness(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    let val = parse_param(params.get(0), 10u8);
    effects::inc_brightness(&mut img, val);
    Ok(img)
}

fn adjust_contrast(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    let val = parse_param(params.get(0), 0.1f32);
    effects::adjust_contrast(&mut img, val);
    Ok(img)
}

fn tint(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 3 {
        return Err(AppError::InvalidActionParam(
            "tint requires 3 parameters: r, g, b".to_string(),
        ));
    }
    let r = parse_param(params.get(0), 0u8) as u32;
    let g = parse_param(params.get(1), 0u8) as u32;
    let b = parse_param(params.get(2), 0u8) as u32;
    effects::tint(&mut img, r.into(), g.into(), b.into());
    Ok(img)
}

fn filter(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.is_empty() {
        return Err(AppError::InvalidActionParam(
            "filter requires 1 parameter: filter_name".to_string(),
        ));
    }
    filters::filter(&mut img, params[0]);
    Ok(img)
}

fn alter_channel(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 2 {
        return Err(AppError::InvalidActionParam(
            "alter_channel requires 2 parameters: channel, amount".to_string(),
        ));
    }
    let channel = parse_param(params.get(0), 0u8);
    let amount = parse_param(params.get(1), 10i16);
    if channel <= 2 {
        channels::alter_channel(&mut img, channel as usize, amount);
    }
    Ok(img)
}

fn swap_channels(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() < 2 {
        return Err(AppError::InvalidActionParam(
            "swap_channels requires 2 parameters: channel1, channel2".to_string(),
        ));
    }
    let channel1 = parse_param(params.get(0), 0u8);
    let channel2 = parse_param(params.get(1), 1u8);
    if channel1 <= 2 && channel2 <= 2 {
        channels::swap_channels(&mut img, channel1 as usize, channel2 as usize);
    }
    Ok(img)
}

fn remove_channel(
    mut img: PhotonImage,
    params: &[&str],
    channel_char: char,
) -> Result<PhotonImage, AppError> {
    let min_filter = parse_param(params.get(0), 255u8);
    match channel_char {
        'r' => channels::remove_red_channel(&mut img, min_filter),
        'g' => channels::remove_green_channel(&mut img, min_filter),
        'b' => channels::remove_blue_channel(&mut img, min_filter),
        _ => { // Explicitly do nothing for unknown channels
        }
    }
    Ok(img)
}
