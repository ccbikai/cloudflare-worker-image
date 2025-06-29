use crate::{actions, encoder, error::AppError};
use actix_web::{http::StatusCode, web};
use bytes::Bytes;
use image::{GenericImageView, ImageBuffer, Rgba};
use log::info;
use photon_rs::PhotonImage;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ImageQuery {
    pub url: String,
    pub action: Option<String>,
    pub format: Option<String>,
    pub quality: Option<u8>,
}

pub async fn process_image(
    query: web::Query<ImageQuery>,
) -> Result<(Vec<u8>, &'static str), AppError> {
    info!("Received new image processing request: {{ url: {}, action: {:?}, format: {:?}, quality: {:?} }}", 
          query.url, query.action, query.format, query.quality);
    // 1. Fetch the image from the URL
    let image_bytes = fetch_image(&query.url).await?;

    // 2. Decode the image and prepare for processing
    let dyn_image = image::load_from_memory(&image_bytes)?;
    let (width, height) = dyn_image.dimensions();
    let raw_pixels = dyn_image.to_rgba8().into_raw();
    let mut img = PhotonImage::new(raw_pixels, width, height);

    // 3. Apply actions if specified
    if let Some(action_str) = &query.action {
        img = actions::apply_actions(img, action_str, &query.url).await?;
    }

    // 4. Encode the image to the desired format
    let format_str = query.format.as_deref().unwrap_or("webp");
    let (output_format, content_type) = encoder::get_image_output_format(format_str);

    let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
        img.get_width(),
        img.get_height(),
        img.get_raw_pixels(),
    )
    .ok_or(AppError::ImageBufferError)?;
    let final_dyn_image = image::DynamicImage::ImageRgba8(image_buffer);

    let encoded_image = encoder::encode_image(final_dyn_image, output_format, query.quality)?;

    Ok((encoded_image, content_type))
}

async fn fetch_image(url: &str) -> Result<Bytes, AppError> {
    let res = reqwest::get(url).await?;

    if !res.status().is_success() {
        return Err(AppError::FetchStatusError {
            url: url.to_string(),
            status: StatusCode::from_u16(res.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        });
    }

    res.bytes().await.map_err(Into::into)
}
