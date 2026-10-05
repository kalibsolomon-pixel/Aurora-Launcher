//! One native boundary for untrusted cosmetic images. Only static PNG is
//! supported by the existing bounded decoder; other formats are rejected.
//! Re-encoding decoded pixels removes source metadata and any alternate browser
//! interpretation. Neither compressed size nor a MIME/signature is validation.

use base64ct::{Base64, Encoding as _};
use std::io::{Cursor, Write};

#[derive(Clone, Copy)]
pub(crate) struct Policy {
    pub(crate) max_encoded_bytes: usize,
    max_dimension: u32,
    max_pixels: u64,
    max_decoded_bytes: usize,
}

pub(crate) const ARTWORK: Policy = Policy {
    max_encoded_bytes: 512 * 1024,
    max_dimension: 1024,
    max_pixels: 1024 * 1024,
    max_decoded_bytes: 16 * 1024 * 1024,
};
pub(crate) const FAVICON: Policy = Policy {
    max_encoded_bytes: 128 * 1024,
    max_dimension: 512,
    max_pixels: 512 * 512,
    max_decoded_bytes: 4 * 1024 * 1024,
};

impl Policy {
    fn surface_bound(self, width: u32, height: u32) -> Option<usize> {
        if width == 0 || height == 0 || width > self.max_dimension || height > self.max_dimension {
            return None;
        }
        let pixels = u64::from(width).checked_mul(u64::from(height))?;
        if pixels > self.max_pixels {
            return None;
        }
        // Even an untransformed RGBA16 PNG has at most eight bytes per pixel.
        let bytes = usize::try_from(pixels.checked_mul(8)?).ok()?;
        (bytes <= self.max_decoded_bytes).then_some(bytes)
    }
}

/// Constructible only after native validation; safe to persist or expose.
pub(crate) struct ValidatedPng(Vec<u8>);
impl ValidatedPng {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn data_url(&self) -> String {
        format!("data:image/png;base64,{}", Base64::encode_string(&self.0))
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let length = self.bytes.len().checked_add(bytes.len());
        if length.is_none_or(|length| length > self.maximum) {
            return Err(std::io::Error::other("cosmetic PNG exceeds encoded limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn validate(bytes: &[u8], policy: Policy) -> Option<ValidatedPng> {
    if bytes.is_empty()
        || bytes.len() > policy.max_encoded_bytes
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
    {
        return None;
    }
    let mut options = png::DecodeOptions::default();
    options.set_ignore_checksums(false);
    // Text/profiles are not needed for cosmetic identity and are never copied.
    options.set_ignore_text_chunk(true);
    options.set_ignore_iccp_chunk(true);
    let mut decoder = png::Decoder::new_with_options(Cursor::new(bytes), options);
    decoder.set_limits(png::Limits {
        bytes: policy.max_decoded_bytes,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    let surface_bound = policy.surface_bound(info.width, info.height)?;
    if info.animation_control.is_some() || info.frame_control.is_some() {
        return None;
    }
    let size = reader.output_buffer_size();
    if size > surface_bound || size > policy.max_decoded_bytes {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(size).ok()?;
    pixels.resize(size, 0);
    let frame = reader.next_frame(&mut pixels).ok()?;
    // Require the rest of the PNG, including IEND, to parse successfully.
    reader.finish().ok()?;
    if reader.info().animation_control.is_some() || reader.info().frame_control.is_some() {
        return None;
    }
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        maximum: policy.max_encoded_bytes,
    };
    {
        let mut encoder = png::Encoder::new(&mut output, frame.width, frame.height);
        encoder.set_color(frame.color_type);
        encoder.set_depth(frame.bit_depth);
        let mut writer = encoder.write_header().ok()?;
        writer
            .write_image_data(&pixels[..frame.buffer_size()])
            .ok()?;
        writer.finish().ok()?;
    }
    Some(ValidatedPng(output.bytes))
}

#[cfg(test)]
pub(crate) mod fixtures {
    pub(crate) const JPEG_4096: &[u8] = include_bytes!("../tests/fixtures/artwork/solid-4096.jpg");
    pub(crate) const JPEG_8192: &[u8] = include_bytes!("../tests/fixtures/artwork/solid-8192.jpg");

    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![127; width as usize * height as usize])
                .unwrap();
            writer.finish().unwrap();
        }
        bytes
    }

    pub(crate) fn animated_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0]).unwrap();
            writer.write_image_data(&[255]).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::*;

    #[test]
    fn static_png_and_exact_dimension_pixel_boundaries_are_supported() {
        for policy in [ARTWORK, FAVICON] {
            for (width, height) in [(32, 32), (policy.max_dimension, policy.max_dimension)] {
                let image = validate(&png(width, height), policy).unwrap();
                let mut reader = png::Decoder::new(Cursor::new(image.bytes()))
                    .read_info()
                    .unwrap();
                assert_eq!((reader.info().width, reader.info().height), (width, height));
                assert!(reader.info().animation_control.is_none());
                let mut pixels = vec![0; reader.output_buffer_size()];
                reader.next_frame(&mut pixels).unwrap();
                reader.finish().unwrap();
            }
            assert!(validate(&png(policy.max_dimension + 1, 1), policy).is_none());
            assert!(validate(&png(1, policy.max_dimension + 1), policy).is_none());
        }
    }

    #[test]
    fn independent_oversized_jpegs_and_signature_only_payloads_are_rejected() {
        assert!(JPEG_4096.len() < FAVICON.max_encoded_bytes);
        assert!(JPEG_8192.len() < ARTWORK.max_encoded_bytes);
        for policy in [ARTWORK, FAVICON] {
            for bytes in [JPEG_4096, JPEG_8192, &JPEG_4096[..32], b"\xFF\xD8\xFF"] {
                assert!(validate(bytes, policy).is_none());
            }
            for bytes in [b"GIF89a".as_slice(), b"RIFF\x00\x00\x00\x00WEBP".as_slice()] {
                assert!(validate(bytes, policy).is_none());
            }
        }
        // Complete 2x2 static and animated GIF/WebP fixtures, not just magic bytes.
        for encoded in [
            "R0lGODlhAgACAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQABQAAACwAAAAAAgACAAAIBgABCAQQEAA7",
            "R0lGODlhAgACAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQABQAAACwAAAAAAgACAAAIBgABCAQQEAAh+QQBBQABACwAAAAAAgACAIEAAP8AAAAAAAAAAAAIBgABCAQQEAA7",
            "UklGRjwAAABXRUJQVlA4IDAAAADQAQCdASoCAAIAAUAmJaACdLoB+AADsAD+8ut//NgVzXPv9//S4P0uD9Lg/9KQAAA=",
            "UklGRsQAAABXRUJQVlA4WAoAAAACAAAAAQAAAQAAQU5JTQYAAAAAAAAAAABBTk1GSgAAAAAAAAAAAAEAAAEAADIAAAJWUDggMgAAADABAJ0BKgIAAgABQCYloAADcAD+8ut///mwP/bz/wR6Af//0uD//pcH//S4P/SkAAAAQU5NRkYAAAAAAAAAAAABAAABAAAyAAAAVlA4IC4AAAA0AQCdASoCAAIAAAAmJaAAA3AA/vtV4///S4P/+lwf/9Lg/9Lg//rV5Vesq6AA",
        ] {
            let bytes = Base64::decode_vec(encoded).unwrap();
            for policy in [ARTWORK, FAVICON] {
                assert!(validate(&bytes, policy).is_none());
            }
        }
    }

    #[test]
    fn normalized_pixels_keep_transparency_without_source_metadata() {
        let mut source = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut source, 1, 1);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![120, 80, 220]);
            encoder.set_trns(vec![127]);
            encoder
                .add_text_chunk("Untrusted".into(), "Not copied to Chromium".into())
                .unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0]).unwrap();
            writer.finish().unwrap();
        }
        let image = validate(&source, ARTWORK).unwrap();
        let mut reader = png::Decoder::new(Cursor::new(image.bytes()))
            .read_info()
            .unwrap();
        assert!(reader.info().uncompressed_latin1_text.is_empty());
        let mut pixels = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(frame.color_type, png::ColorType::Rgba);
        assert_eq!(pixels, [120, 80, 220, 127]);
    }

    #[test]
    fn malformed_truncated_corrupt_and_animated_pngs_are_rejected() {
        let valid = png(32, 32);
        let mut corrupt = valid.clone();
        corrupt[29] ^= 1; // IHDR checksum
        for policy in [ARTWORK, FAVICON] {
            for bytes in [
                b"\x89PNG\r\n\x1a\n".as_slice(),
                &valid[..valid.len() - 12],
                &corrupt,
                &animated_png(),
            ] {
                assert!(validate(bytes, policy).is_none());
            }
        }
    }

    #[test]
    fn encoded_limits_apply_to_input_and_canonical_output() {
        let bytes = png(32, 32);
        let exact = Policy {
            max_encoded_bytes: bytes.len(),
            ..ARTWORK
        };
        assert!(validate(&bytes, exact).is_some());
        assert!(
            validate(
                &bytes,
                Policy {
                    max_encoded_bytes: bytes.len() - 1,
                    ..exact
                }
            )
            .is_none()
        );
        for policy in [ARTWORK, FAVICON] {
            assert!(validate(&vec![0; policy.max_encoded_bytes + 1], policy).is_none());
        }
        let mut output = BoundedOutput {
            bytes: Vec::new(),
            maximum: 2,
        };
        assert!(output.write_all(&[1, 2]).is_ok());
        assert!(output.write_all(&[3]).is_err());
        assert_eq!(output.bytes, [1, 2]);
    }

    #[test]
    fn pixel_memory_zero_dimension_and_overflow_limits_fail_closed() {
        let bytes = png(32, 32);
        assert!(
            validate(
                &bytes,
                Policy {
                    max_pixels: 1023,
                    ..ARTWORK
                }
            )
            .is_none()
        );
        assert!(
            validate(
                &bytes,
                Policy {
                    max_decoded_bytes: 32 * 32 * 8 - 1,
                    ..ARTWORK
                }
            )
            .is_none()
        );
        assert!(ARTWORK.surface_bound(0, 1).is_none());
        assert!(ARTWORK.surface_bound(1, 0).is_none());
        let extreme = Policy {
            max_dimension: u32::MAX,
            max_pixels: u64::MAX,
            max_decoded_bytes: usize::MAX,
            ..ARTWORK
        };
        assert!(extreme.surface_bound(u32::MAX, u32::MAX).is_none());
    }
}
