//! Bounded retained artwork for Content rows and Discover result cards.
//!
//! The core owns every artwork read (installed archive icons, cached provider
//! project icons); the presentation never fetches an image itself. Workers
//! decode what the core returns into [`RawArtwork`] pixels, and the UI thread
//! converts each image exactly once with [`crate::images::rgba_image`] and
//! retains the `RenderImage` here, so rendering only clones an `Arc`.
//!
//! Each view owns one [`ArtworkCache`]. It is insertion-ordered and hard
//! capped: when full, the oldest entry the caller does not currently want is
//! evicted first. A known-absent icon is cached as `None` so it is not
//! requested again until the owner invalidates it.
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

use gpui::{
    Div, InteractiveElement as _, ParentElement as _, Pixels, RenderImage, Stateful, Styled as _,
    div, img, px,
};

use crate::{api, images, theme::AuroraTheme};

/// Largest side of a core-provided archive icon bitmap.
const MAX_BITMAP_SIDE: u32 = 96;
/// Largest provider artwork PNG the core hands over.
const MAX_PNG_BYTES: usize = 2 * 1024 * 1024;

/// Decoded, not yet GPU-converted pixels, safe to build on a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawArtwork {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RawArtwork {
    /// The single BGRA conversion; `None` for malformed or oversized pixels.
    pub fn into_image(self) -> Option<Arc<RenderImage>> {
        images::rgba_image(self.width, self.height, self.rgba)
    }
}

/// Archive icon from the core's batched icon calls: bounded base64 RGBA.
pub fn raw_from_bitmap(bitmap: &api::ModIconBitmap) -> Option<RawArtwork> {
    use base64::Engine;
    if bitmap.width == 0
        || bitmap.height == 0
        || bitmap.width > MAX_BITMAP_SIDE
        || bitmap.height > MAX_BITMAP_SIDE
    {
        return None;
    }
    let rgba = base64::engine::general_purpose::STANDARD
        .decode(&bitmap.rgba_base64)
        .ok()?;
    (rgba.len() == bitmap.width as usize * bitmap.height as usize * 4).then_some(RawArtwork {
        width: bitmap.width,
        height: bitmap.height,
        rgba,
    })
}

/// Provider artwork: the normalized PNG from the core's isolated decoder.
pub fn raw_from_png(bytes: &[u8]) -> Option<RawArtwork> {
    if bytes.is_empty() || bytes.len() > MAX_PNG_BYTES {
        return None;
    }
    let rgba = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = rgba.dimensions();
    Some(RawArtwork {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

/// The first alphanumeric character, uppercased, as a placeholder glyph.
pub fn letter(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

/// Insertion-ordered, hard-capped image cache. `Some(None)` from [`get`]
/// means the icon is known to be absent.
///
/// [`get`]: ArtworkCache::get
pub struct ArtworkCache<K: Eq + Hash + Clone> {
    images: HashMap<K, Option<Arc<RenderImage>>>,
    order: VecDeque<K>,
    capacity: usize,
}

impl<K: Eq + Hash + Clone> ArtworkCache<K> {
    pub fn new(capacity: usize) -> Self {
        Self {
            images: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn contains(&self, key: &K) -> bool {
        self.images.contains_key(key)
    }

    pub fn get(&self, key: &K) -> Option<&Option<Arc<RenderImage>>> {
        self.images.get(key)
    }

    /// The retained image, if the icon exists. A clone of an `Arc` only.
    pub fn image(&self, key: &K) -> Option<Arc<RenderImage>> {
        self.images.get(key).cloned().flatten()
    }

    /// Records a result, then evicts down to the cap. The oldest entry that
    /// `wanted` does not claim goes first; only when every other entry is
    /// wanted does the oldest wanted one go. The inserted key itself stays.
    pub fn insert(&mut self, key: K, image: Option<Arc<RenderImage>>, wanted: impl Fn(&K) -> bool) {
        self.order.retain(|old| old != &key);
        self.order.push_back(key.clone());
        self.images.insert(key.clone(), image);
        while self.images.len() > self.capacity {
            let victim = self
                .order
                .iter()
                .position(|old| old != &key && !wanted(old))
                .or_else(|| self.order.iter().position(|old| old != &key));
            let Some(victim) = victim.and_then(|index| self.order.remove(index)) else {
                break;
            };
            self.images.remove(&victim);
        }
    }

    pub fn retain(&mut self, keep: impl Fn(&K) -> bool) {
        self.images.retain(|key, _| keep(key));
        self.order.retain(|key| self.images.contains_key(key));
    }

    pub fn clear(&mut self) {
        self.images.clear();
        self.order.clear();
    }
}

/// A square artwork tile: the retained image, or a letter glyph while the
/// icon loads or when none exists.
pub fn tile(
    id: impl Into<gpui::ElementId>,
    image: Option<Arc<RenderImage>>,
    glyph: &str,
    side: Pixels,
    palette: &AuroraTheme,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .size(side)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded(palette.radius)
        .bg(palette.surface_hover);
    match image {
        Some(image) => base.child(img(image).size(side)),
        None => base
            .text_size(px(f32::from(side) * 0.45))
            .text_color(palette.accent)
            .child(glyph.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(seed: u8) -> Option<Arc<RenderImage>> {
        images::rgba_image(1, 1, vec![seed, 0, 0, 255])
    }

    #[test]
    fn cache_evicts_oldest_unwanted_first_and_never_exceeds_its_cap() {
        let mut cache = ArtworkCache::new(3);
        for key in ["a", "b", "c"] {
            cache.insert(key.to_owned(), image(1), |_| false);
        }
        // "a" is wanted by the current view, so the older unwanted "b" goes.
        cache.insert("d".to_owned(), image(2), |key| key == "a");
        assert_eq!(cache.len(), 3);
        assert!(cache.contains(&"a".to_owned()));
        assert!(!cache.contains(&"b".to_owned()));
        // When every other entry is wanted the oldest still yields; the
        // inserted entry is never its own victim.
        cache.insert("e".to_owned(), image(3), |_| true);
        assert_eq!(cache.len(), 3);
        assert!(!cache.contains(&"a".to_owned()));
        assert!(cache.contains(&"e".to_owned()));
    }

    #[test]
    fn reinserting_refreshes_order_without_growing() {
        let mut cache = ArtworkCache::new(2);
        cache.insert("a".to_owned(), image(1), |_| false);
        cache.insert("b".to_owned(), image(1), |_| false);
        cache.insert("a".to_owned(), image(2), |_| false);
        assert_eq!(cache.len(), 2);
        cache.insert("c".to_owned(), image(1), |_| false);
        assert!(cache.contains(&"a".to_owned()));
        assert!(!cache.contains(&"b".to_owned()));
    }

    #[test]
    fn absent_icons_are_remembered_and_retain_prunes_order_too() {
        let mut cache = ArtworkCache::new(4);
        cache.insert("none".to_owned(), None, |_| false);
        cache.insert("some".to_owned(), image(1), |_| false);
        assert!(matches!(cache.get(&"none".to_owned()), Some(None)));
        assert!(cache.image(&"none".to_owned()).is_none());
        assert!(cache.image(&"some".to_owned()).is_some());
        cache.retain(|key| key == "some");
        assert!(!cache.contains(&"none".to_owned()));
        assert_eq!(cache.order.len(), 1);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.order.is_empty());
    }

    #[test]
    fn a_retained_image_is_shared_not_reconverted() {
        let mut cache = ArtworkCache::new(2);
        cache.insert("a".to_owned(), image(1), |_| false);
        let first = cache.image(&"a".to_owned()).unwrap();
        let second = cache.image(&"a".to_owned()).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn archive_bitmaps_are_bounded_and_length_checked() {
        use base64::Engine;
        let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let bitmap = |width, height, bytes: &[u8]| api::ModIconBitmap {
            width,
            height,
            rgba_base64: encode(bytes),
        };
        let ok = raw_from_bitmap(&bitmap(1, 2, &[0; 8])).unwrap();
        assert_eq!((ok.width, ok.height, ok.rgba.len()), (1, 2, 8));
        assert!(raw_from_bitmap(&bitmap(0, 1, &[])).is_none());
        assert!(raw_from_bitmap(&bitmap(97, 1, &vec![0; 97 * 4])).is_none());
        assert!(raw_from_bitmap(&bitmap(1, 1, &[0; 3])).is_none());
        assert!(
            raw_from_bitmap(&api::ModIconBitmap {
                width: 1,
                height: 1,
                rgba_base64: "!!not base64!!".into(),
            })
            .is_none()
        );
    }

    #[test]
    fn provider_png_decodes_once_and_rejects_garbage_and_oversize() {
        assert!(raw_from_png(b"not a png").is_none());
        assert!(raw_from_png(&[]).is_none());
        assert!(raw_from_png(&vec![0; MAX_PNG_BYTES + 1]).is_none());
    }

    #[test]
    fn placeholder_glyph_is_the_first_alphanumeric_uppercased() {
        assert_eq!(letter("sodium"), "S");
        assert_eq!(letter("  - 3D Pack"), "3");
        assert_eq!(letter("★ ★"), "?");
        assert_eq!(letter(""), "?");
    }
}
