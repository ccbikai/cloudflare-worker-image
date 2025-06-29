use crate::error::AppError;
use crate::utils::parse_param;
use image::GenericImageView;
use photon_rs::{channels, conv, effects, filters, monochrome, multiple, text, transform, PhotonImage};
use reqwest;

pub async fn apply_actions(
    mut img: PhotonImage,
    action_str: &str,
    original_url: &str,
) -> Result<PhotonImage, AppError> {
    let actions = action_str.split('|').filter(|s| !s.is_empty());

    for action_item in actions {
        let parts: Vec<&str> = action_item.split('!').collect();
        let action_name = parts[0];
        let options = if parts.len() > 1 { parts[1] } else { "" };
        let params: Vec<&str> = options.split(',').filter(|s| !s.is_empty()).collect();

        println!(
            "[LOG] url: {} Applying action: {} with params {:?}",
            original_url,
            action_name,
            params
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
                println!(
                    "[WARN] url: {} Unknown action: {}",
                    original_url,
                    action_name
                );
                img
            }
        };
    }
    Ok(img)
}

fn resize(img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
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
        Ok(transform::resize(&img, width, height, filter))
    } else {
        Err(AppError::InvalidActionParam(
            "resize requires at least 2 parameters: width, height".to_string(),
        ))
    }
}

fn crop(img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() >= 4 {
        let x1 = parse_param(params.get(0), 0);
        let y1 = parse_param(params.get(1), 0);
        let x2 = parse_param(params.get(2), img.get_width());
        let y2 = parse_param(params.get(3), img.get_height());
        Ok(transform::crop(&img, x1, y1, x2, y2))
    } else {
        Err(AppError::InvalidActionParam(
            "crop requires 4 parameters: x1, y1, x2, y2".to_string(),
        ))
    }
}

fn rotate(img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if !params.is_empty() {
        let angle = parse_param(params.get(0), 90i32);
        Ok(transform::rotate(&img, angle as f32))
    } else {
        Err(AppError::InvalidActionParam(
            "rotate requires 1 parameter: angle".to_string(),
        ))
    }
}

async fn watermark(
    mut img: PhotonImage,
    params: &[&str],
) -> Result<PhotonImage, AppError> {
    if !params.is_empty() {
        let watermark_url = params[0];
        let x = parse_param(params.get(1), 0);
        let y = parse_param(params.get(2), 0);

        let wm_res = reqwest::get(watermark_url)
            .await
            .map_err(AppError::WatermarkFetchError)?;
        let wm_bytes = wm_res.bytes().await.map_err(AppError::WatermarkFetchError)?;
        let watermark_dyn_img =
            image::load_from_memory(&wm_bytes).map_err(AppError::WatermarkDecodeError)?;

        let (width, height) = watermark_dyn_img.dimensions();
        let raw_pixels = watermark_dyn_img.to_rgba8().into_raw();
        let watermark_img = PhotonImage::new(raw_pixels, width, height);

        multiple::watermark(&mut img, &watermark_img, x, y);
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "watermark requires at least 1 parameter: watermark_url".to_string(),
        ))
    }
}

async fn blend(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() >= 2 {
        let blend_url = params[0];
        let blend_mode = params[1];

        let top_res = reqwest::get(blend_url)
            .await
            .map_err(AppError::WatermarkFetchError)?;
        let top_bytes = top_res.bytes().await.map_err(AppError::WatermarkFetchError)?;
        let top_dyn_img =
            image::load_from_memory(&top_bytes).map_err(AppError::WatermarkDecodeError)?;

        let (width, height) = top_dyn_img.dimensions();
        let raw_pixels = top_dyn_img.to_rgba8().into_raw();
        let top_img = PhotonImage::new(raw_pixels, width, height);

        multiple::blend(&mut img, &top_img, blend_mode);
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "blend requires 2 parameters: blend_url, blend_mode".to_string(),
        ))
    }
}

fn draw_text(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() >= 3 {
        let text_content = params[0];
        let x = parse_param(params.get(1), 0i32);
        let y = parse_param(params.get(2), 0i32);
        let font_size = parse_param(params.get(3), 24.0f32);
        text::draw_text(&mut img, text_content, x, y, font_size);
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "draw_text requires at least 3 parameters: text, x, y".to_string(),
        ))
    }
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
    if params.len() >= 3 {
        let r = parse_param(params.get(0), 0u8);
        let g = parse_param(params.get(1), 0u8);
        let b = parse_param(params.get(2), 0u8);
        effects::tint(&mut img, r.into(), g.into(), b.into());
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "tint requires 3 parameters: r, g, b".to_string(),
        ))
    }
}

fn filter(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if !params.is_empty() {
        filters::filter(&mut img, params[0]);
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "filter requires 1 parameter: filter_name".to_string(),
        ))
    }
}

fn alter_channel(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() >= 2 {
        let channel = parse_param(params.get(0), 0u8);
        let amount = parse_param(params.get(1), 10i16);
        if channel <= 2 {
            channels::alter_channel(&mut img, channel as usize, amount);
        }
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "alter_channel requires 2 parameters: channel, amount".to_string(),
        ))
    }
}

fn swap_channels(mut img: PhotonImage, params: &[&str]) -> Result<PhotonImage, AppError> {
    if params.len() >= 2 {
        let channel1 = parse_param(params.get(0), 0u8);
        let channel2 = parse_param(params.get(1), 1u8);
        if channel1 <= 2 && channel2 <= 2 {
            channels::swap_channels(&mut img, channel1 as usize, channel2 as usize);
        }
        Ok(img)
    } else {
        Err(AppError::InvalidActionParam(
            "swap_channels requires 2 parameters: channel1, channel2".to_string(),
        ))
    }
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
        _ => {}
    }
    Ok(img)
}
