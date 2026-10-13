use crate::app::config::StickerConfig;
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

pub(super) fn with_metadata(
    bytes: &[u8],
    config: &StickerConfig,
    animated: bool,
) -> Result<Vec<u8>> {
    let limit = if animated { 500 * 1024 } else { 100 * 1024 };
    ensure!(bytes.len() <= limit, "Sticker exceeds size limit");
    ensure!(
        bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "Invalid WebP header"
    );
    let declared = u32::from_le_bytes(bytes[4..8].try_into()?) as u64 + 8;
    ensure!(declared == bytes.len() as u64, "Invalid WebP length");
    let first = bytes.get(12..16).context("WebP image missing")?;
    ensure!(
        matches!(first, b"VP8X" | b"VP8 " | b"VP8L"),
        "Invalid first WebP chunk"
    );

    let mut output = b"RIFF\0\0\0\0WEBP".to_vec();
    if first != b"VP8X" {
        // FFmpeg's sticker filter always produces a 512 x 512 canvas.
        chunk(
            &mut output,
            b"VP8X",
            &[0x08, 0, 0, 0, 0xff, 1, 0, 0xff, 1, 0],
        );
    }
    let mut offset = 12;
    let mut has_image = false;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 8)
            .context("Truncated WebP chunk")?;
        let size = u32::from_le_bytes(header[4..8].try_into()?) as usize;
        let end = offset
            .checked_add(8)
            .and_then(|n| n.checked_add(size))
            .and_then(|n| n.checked_add(size % 2))
            .context("WebP chunk size overflow")?;
        let data = bytes.get(offset..end).context("Truncated WebP payload")?;
        match &header[..4] {
            b"VP8X" => {
                ensure!(offset == 12 && size == 10, "Invalid VP8X chunk");
                output.extend_from_slice(data);
                output[20] |= 0x08;
            }
            b"EXIF" => (),
            _ => {
                if matches!(&header[..4], b"VP8 " | b"VP8L" | b"ANMF") {
                    ensure!(size > 0, "Empty WebP image chunk");
                    has_image = true;
                }
                if first == b"VP8L" && offset == 12 {
                    ensure!(size >= 5, "Truncated VP8L header");
                    output[20] |= data[12] & 0x10;
                }
                output.extend_from_slice(data);
            }
        }
        offset = end;
    }
    ensure!(has_image, "WebP image missing");

    let pack_id = hex::encode(Sha256::digest(serde_json::to_vec(&(
        &config.packname,
        &config.authorname,
    ))?));
    let json = serde_json::to_vec(&serde_json::json!({
        "sticker-pack-id": pack_id,
        "sticker-pack-name": config.packname,
        "sticker-pack-publisher": config.authorname,
    }))?;
    // WhatsApp stores pack JSON in the little-endian TIFF tag 0x5741.
    let mut exif = vec![0x49, 0x49, 0x2a, 0, 8, 0, 0, 0, 1, 0, 0x41, 0x57, 7, 0];
    exif.extend_from_slice(&u32::try_from(json.len())?.to_le_bytes());
    exif.extend_from_slice(&22_u32.to_le_bytes());
    exif.extend_from_slice(&json);
    chunk(&mut output, b"EXIF", &exif);
    ensure!(
        output.len() <= limit,
        "Sticker with metadata exceeds size limit; use a shorter/simpler source"
    );
    let size = u32::try_from(output.len() - 8)?;
    output[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(output)
}

fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(kind);
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(data);
    if !data.len().is_multiple_of(2) {
        output.push(0);
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn webp(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
        for (kind, data) in chunks {
            chunk(&mut bytes, kind, data);
        }
        let len = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&len.to_le_bytes());
        bytes
    }

    pub(crate) fn assert_metadata(bytes: &[u8], config: &StickerConfig) {
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
            bytes.len()
        );
        assert_eq!(&bytes[12..16], b"VP8X");
        assert_ne!(bytes[20] & 0x08, 0);
        let mut offset = 12;
        let mut found = 0;
        while offset < bytes.len() {
            let size =
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
            if &bytes[offset..offset + 4] == b"EXIF" {
                found += 1;
                let exif = &bytes[offset + 8..offset + 8 + size];
                assert_eq!(
                    &exif[..14],
                    &[0x49, 0x49, 0x2a, 0, 8, 0, 0, 0, 1, 0, 0x41, 0x57, 7, 0]
                );
                assert_eq!(
                    u32::from_le_bytes(exif[14..18].try_into().unwrap()) as usize,
                    exif.len() - 22
                );
                assert_eq!(u32::from_le_bytes(exif[18..22].try_into().unwrap()), 22);
                let json: serde_json::Value = serde_json::from_slice(&exif[22..]).unwrap();
                assert_eq!(json["sticker-pack-name"], config.packname);
                assert_eq!(json["sticker-pack-publisher"], config.authorname);
                assert_eq!(json["sticker-pack-id"].as_str().unwrap().len(), 64);
            }
            offset += 8 + size + size % 2;
        }
        assert_eq!(found, 1);
        assert_eq!(offset, bytes.len());
    }

    #[test]
    fn embeds_unicode_names_and_replaces_old_metadata() {
        let config = StickerConfig {
            packname: "Stiker 😎 \"bot\"".into(),
            authorname: "名前".into(),
        };
        let image = webp(&[(b"VP8 ", b"abc")]);
        let result = with_metadata(&image, &config, false).unwrap();
        assert_metadata(&result, &config);
        assert_eq!(&result[30..42], &image[12..24]);
        assert_eq!(result.len() % 2, 0);
        assert_eq!(with_metadata(&result, &config, false).unwrap(), result);
        let changed = StickerConfig {
            packname: "New pack".into(),
            authorname: "Other author".into(),
        };
        assert_metadata(&with_metadata(&result, &changed, false).unwrap(), &changed);
    }

    #[test]
    fn preserves_animation_alpha_and_lossless_data() {
        let config = StickerConfig::default();
        let extended = [0x16, 0, 0, 0, 0xff, 1, 0, 0xff, 1, 0];
        let image = webp(&[
            (b"VP8X", &extended),
            (b"ANIM", &[0; 6]),
            (b"ANMF", b"frame"),
            (b"EXIF", b"old"),
        ]);
        let result = with_metadata(&image, &config, true).unwrap();
        assert_metadata(&result, &config);
        assert_eq!(result[20], 0x1e);
        assert_eq!(&result[21..58], &image[21..58]);
        let lossless = webp(&[(b"VP8L", &[0x2f, 0, 0, 0, 0x10])]);
        let result = with_metadata(&lossless, &config, false).unwrap();
        assert_eq!(result[20], 0x18);
        assert_metadata(&result, &config);
    }

    #[test]
    fn rejects_truncated_containers_and_metadata_over_budget() {
        let config = StickerConfig::default();
        let image = webp(&[(b"VP8 ", b"abc")]);
        for end in 0..image.len() {
            assert!(with_metadata(&image[..end], &config, false).is_err());
        }
        let mut malformed = image.clone();
        malformed[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(with_metadata(&malformed, &config, false).is_err());
        assert!(with_metadata(&webp(&[(b"VP8X", &[0; 10])]), &config, false).is_err());
        assert!(with_metadata(&webp(&[(b"VP8X", &[0; 9])]), &config, false).is_err());
        for (animated, limit) in [(false, 100 * 1024), (true, 500 * 1024)] {
            let full = webp(&[(b"VP8 ", &vec![0; limit - 20])]);
            assert_eq!(full.len(), limit);
            assert!(with_metadata(&full, &config, animated).is_err());
        }
    }
}
