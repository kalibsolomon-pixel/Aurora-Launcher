//! Convert decoded core artwork once, then retain the GPU image across renders.
#![allow(dead_code)]

use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};

/// GPUI consumes BGRA frames. Reject malformed or oversized decoded artwork
/// before transferring ownership; this does not decode untrusted image files.
pub fn rgba_image(width: u32, height: u32, mut rgba: Vec<u8>) -> Option<Arc<RenderImage>> {
    let pixels = width.checked_mul(height)?;
    if pixels == 0 || pixels > 4 * 1024 * 1024 {
        return None;
    }
    let bytes = usize::try_from(pixels).ok()?.checked_mul(4)?;
    if rgba.len() != bytes {
        return None;
    }
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let frame = Frame::new(RgbaImage::from_raw(width, height, rgba)?);
    Some(Arc::new(RenderImage::new(vec![frame])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_artwork_preserves_alpha_and_swaps_red_and_blue_for_gpui() {
        let image = rgba_image(2, 1, vec![1, 2, 3, 4, 11, 12, 13, 14]).unwrap();
        assert_eq!(image.as_bytes(0).unwrap(), &[3, 2, 1, 4, 13, 12, 11, 14]);
        assert_eq!(image.frame_count(), 1);
        assert_eq!(image.size(0).width.0, 2);
        assert_eq!(image.size(0).height.0, 1);
    }

    #[test]
    fn invalid_decoded_dimensions_and_pixel_counts_are_rejected() {
        for (width, height, bytes) in [
            (0, 1, vec![]),
            (1, 0, vec![]),
            (1, 1, vec![0; 3]),
            (1, 1, vec![0; 5]),
            (u32::MAX, u32::MAX, vec![]),
            (4096, 4096, vec![]),
        ] {
            assert!(rgba_image(width, height, bytes).is_none());
        }
    }
}
