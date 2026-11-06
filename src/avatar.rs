//! Reviewable avatar preparation and the official FILE multipart adapters for user/group avatars.
use crate::session::Session;
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::path::Path;
/// Avatar destination is explicit and separate from sending a chat attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Own,
    Group(String),
}
/// The reviewed preview and uploaded JPEG come from the same prepared bytes.
#[derive(Clone)]
pub struct Draft {
    pub jpeg: Vec<u8>,
    pub rgba: Vec<u8>,
    pub side: u32,
}
/// Decode/crop on a blocking worker, never on the render thread.
/// # Errors
/// Rejects non-files, oversized input, invalid formats and decode/allocation bombs.
pub fn prepare(path: &Path) -> Result<Draft> {
    use image::ImageDecoder;
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() <= 16 * 1024 * 1024,
        "Ảnh đại diện phải là tệp không quá 16 MB"
    );
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Ảnh thay đổi trong lúc đọc hoặc vượt 16 MB"
    );
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    ensure!(
        width > 0 && height > 0 && u64::from(width) * u64::from(height) <= 16_000_000,
        "Kích thước ảnh đại diện quá lớn"
    );
    let orientation = decoder.orientation().ok();
    let mut image = image::DynamicImage::from_decoder(decoder)?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    let size = image.width().min(image.height());
    let cropped = image
        .crop_imm(
            (image.width() - size) / 2,
            (image.height() - size) / 2,
            size,
            size,
        )
        .resize_exact(512, 512, image::imageops::FilterType::CatmullRom)
        .to_rgba8();
    let mut rgb = image::RgbImage::new(512, 512);
    for (out, source) in rgb.pixels_mut().zip(cropped.pixels()) {
        let a = u32::from(source[3]);
        for channel in 0..3 {
            out[channel] = ((u32::from(source[channel]) * a + 255 * (255 - a) + 127) / 255) as u8;
        }
    }
    let image = image::DynamicImage::ImageRgb8(rgb);
    let rgba = image.to_rgba8().into_raw();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90).encode_image(&image)?;
    Ok(Draft {
        jpeg,
        rgba,
        side: 512,
    })
}
/// Build the observed 160px avatar request; processed dimensions describe the reviewed image.
/// # Errors
/// Rejects invalid group/operation identities and malformed drafts.
pub fn params(
    s: &Session,
    target: &Target,
    draft: &Draft,
    client: &str,
) -> Result<(&'static str, Value)> {
    ensure!(
        !client.is_empty()
            && client.len() <= 128
            && client.bytes().all(|b| b.is_ascii_alphanumeric()),
        "Thiếu định danh thay ảnh đại diện"
    );
    ensure!(
        draft.side == 512 && draft.jpeg.len() <= 2 * 1024 * 1024 && !draft.jpeg.is_empty(),
        "Bản xem trước ảnh đại diện không hợp lệ"
    );
    match target {
        Target::Own => Ok((
            "/api/profile/upavatar",
            json!({"avatarSize":160,"clientId":client,"language":"vi"}),
        )),
        Target::Group(group) => {
            let id = crate::server_pins::identity(group, true)?;
            Ok((
                "/api/group/upavatar",
                json!({"grid":&id[1..],"avatarSize":160,"clientId":client,"originWidth":draft.side,"originHeight":draft.side,"imei":s.imei}),
            ))
        }
    }
}
/// Upload once after permission refresh. An ambiguous result must not re-upload automatically.
/// # Errors
/// Returns permission, account, multipart/transport or explicit server failures.
pub async fn upload(
    s: &Session,
    target: &Target,
    draft: &Draft,
    client: &str,
) -> Result<Option<crate::profile::Snapshot>> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let (path, body) = params(s, target, draft, client)?;
    if let Target::Group(group) = target {
        let snapshot = crate::group_state::snapshot(s, group, true).await?;
        ensure!(!snapshot.removed, "Bạn không còn trong nhóm");
        ensure!(
            snapshot.manager(s.account_key())
                || snapshot
                    .info
                    .pointer("/setting/blockName")
                    .and_then(crate::api_response::integer)
                    == Some(0),
            "Nhóm hạn chế thay ảnh đại diện"
        );
    }
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    let encoded = s.encoded_params(&body)?;
    let response = s
        .post_multipart(
            &format!(
                "{}{path}?{}&params={encoded}",
                s.domains.file,
                s.common_params()
            ),
            "fileContent",
            "avatar.jpg",
            "image/jpeg",
            &draft.jpeg,
        )
        .await;
    if let Target::Group(group) = target {
        crate::group_state::invalidate(s, Some(group));
        if let Err(error) = crate::group_metadata::invalidate(s, group).await {
            crate::logger::log_error("group avatar cache invalidation", &error.to_string());
        }
    }
    let raw = s.maybe_decrypt(response?, s.session_key_bytes()).await;
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận ảnh đại diện từ máy chủ"
    );
    crate::api_response::checked_data(raw)?;
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    if matches!(target, Target::Own) {
        Ok(Some(
            crate::profile::execute(s, &crate::profile::Action::Read).await?,
        ))
    } else {
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_crops_and_composites_the_same_preview_that_is_uploaded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        image::RgbaImage::from_pixel(16, 8, image::Rgba([255, 0, 0, 128]))
            .save(&path)
            .unwrap();
        let draft = prepare(&path).unwrap();
        assert_eq!(draft.rgba.len(), 512 * 512 * 4);
        assert_eq!(draft.side, 512);
        let jpeg = image::load_from_memory(&draft.jpeg).unwrap();
        assert_eq!((jpeg.width(), jpeg.height()), (512, 512));
        let s = Session::new(false, false, false).unwrap();
        let (path, body) = params(&s, &Target::Group("g123".into()), &draft, "op123").unwrap();
        assert_eq!(path, "/api/group/upavatar");
        assert_eq!(body["grid"], "123");
        assert_eq!(body["avatarSize"], 160);
        assert_eq!(body["originWidth"], 512);
        assert!(prepare(root.path()).is_err());
    }
}
