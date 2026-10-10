//! One native boundary for untrusted cosmetic images. Static PNG remains the
//! display/cache format. Provider artwork additionally decodes static WebP and
//! a GIF's first frame in a byte-only, OS-memory-limited disposable worker;
//! other formats are rejected.
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

/// Provider artwork additionally admits static WebP and static GIF snapshots.
/// Browser input and stored output remain canonical PNG; server favicons retain
/// their PNG-only policy.
pub(crate) fn validate_artwork(bytes: &[u8]) -> Option<ValidatedPng> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return validate(bytes, ARTWORK);
    }
    if bytes.starts_with(b"GIF") {
        gif_dimensions(bytes)?;
    } else {
        webp_dimensions(bytes)?;
    }
    #[cfg(test)]
    return decode_provider_image(bytes);
    #[cfg(not(test))]
    isolated_provider_image(bytes)
}

fn decode_provider_image(bytes: &[u8]) -> Option<ValidatedPng> {
    if bytes.starts_with(b"GIF") {
        decode_gif(bytes)
    } else {
        decode_webp(bytes)
    }
}

fn gif_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 14
        || bytes.len() > ARTWORK.max_encoded_bytes
        || !matches!(&bytes[..6], b"GIF87a" | b"GIF89a")
        || bytes.last() != Some(&0x3b)
    {
        return None;
    }
    let width = u32::from(u16::from_le_bytes(bytes[6..8].try_into().ok()?));
    let height = u32::from(u16::from_le_bytes(bytes[8..10].try_into().ok()?));
    ARTWORK.surface_bound(width, height)?;
    Some((width, height))
}

// Freeze the real first frame rather than sending animation to the browser.
// Decode all frames to reject damaged/truncated streams, without retaining or
// compositing the later frames. Both count and total decoded work are bounded.
fn decode_gif(bytes: &[u8]) -> Option<ValidatedPng> {
    let (width, height) = gif_dimensions(bytes)?;
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    options.set_memory_limit(gif::MemoryLimit::Bytes(std::num::NonZeroU64::new(
        ARTWORK.max_decoded_bytes as u64,
    )?));
    options.check_frame_consistency(true);
    // Keep the decoder's standard GIF compatibility: Enchant Icons omits the
    // final LZW end code but has complete pixels, image blocks and GIF trailer.
    // Frame/canvas bounds, full stream decoding and worker budgets still apply.
    let mut decoder = options.read_info(Cursor::new(bytes)).ok()?;
    if (u32::from(decoder.width()), u32::from(decoder.height())) != (width, height) {
        return None;
    }
    let mut pixels = Vec::new();
    let mut frames = 0;
    let mut decoded = 0usize;
    while let Some(frame) = decoder.read_next_frame().ok()? {
        frames += 1;
        decoded = decoded.checked_add(frame.buffer.len())?;
        ARTWORK.surface_bound(u32::from(frame.width), u32::from(frame.height))?;
        if frames > 64 || decoded > ARTWORK.max_decoded_bytes {
            return None;
        }
        if frames == 1 {
            let size = usize::try_from(u64::from(width) * u64::from(height) * 4).ok()?;
            pixels.try_reserve_exact(size).ok()?;
            pixels.resize(size, 0);
            // A GIF's first image can cover only part of its logical screen.
            // The untouched canvas stays transparent, preserving icon alpha.
            let stride = usize::from(frame.width) * 4;
            if frame.buffer.len() != stride * usize::from(frame.height) {
                return None;
            }
            for row in 0..usize::from(frame.height) {
                let destination =
                    ((usize::from(frame.top) + row) * width as usize + usize::from(frame.left)) * 4;
                pixels
                    .get_mut(destination..destination + stride)?
                    .copy_from_slice(&frame.buffer[row * stride..(row + 1) * stride]);
            }
        }
    }
    if frames == 0 {
        return None;
    }
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        maximum: ARTWORK.max_encoded_bytes,
    };
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
        writer.finish().ok()?;
    }
    validate(&output.bytes, ARTWORK)
}

fn decode_webp(bytes: &[u8]) -> Option<ValidatedPng> {
    if bytes.len() < 12
        || bytes.len() > ARTWORK.max_encoded_bytes
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WEBP"
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize != bytes.len() - 8
    {
        return None;
    }
    let expected = webp_dimensions(bytes)?;
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
    decoder.set_memory_limit(ARTWORK.max_decoded_bytes);
    let (width, height) = decoder.dimensions();
    if (width, height) != expected {
        return None;
    }
    let bound = ARTWORK.surface_bound(width, height)?;
    if decoder.is_animated() {
        return None;
    }
    let size = decoder.output_buffer_size()?;
    if size > bound {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(size).ok()?;
    pixels.resize(size, 0);
    decoder.read_image(&mut pixels).ok()?;
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        maximum: ARTWORK.max_encoded_bytes,
    };
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(if decoder.has_alpha() {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
        writer.finish().ok()?;
    }
    validate(&output.bytes, ARTWORK)
}

// Check every RIFF chunk BEFORE constructing the decoder. In particular an
// extended canvas must not hide an oversized VP8/VP8L bitstream: some decoder
// scratch allocations precede its final canvas-consistency check.
fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 12
        || bytes.len() > ARTWORK.max_encoded_bytes
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WEBP"
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize != bytes.len() - 8
    {
        return None;
    }
    let mut offset: usize = 12;
    let mut canvas = None;
    let mut image = None;
    let mut chunks = 0;
    while offset < bytes.len() {
        chunks += 1;
        if chunks > 64 {
            return None;
        }
        let header = bytes.get(offset..offset.checked_add(8)?)?;
        let size = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        let start = offset.checked_add(8)?;
        let end = start.checked_add(size)?;
        let data = bytes.get(start..end)?;
        let dimensions = match &header[..4] {
            b"ANIM" | b"ANMF" => return None,
            b"VP8X" => {
                if canvas.is_some() || image.is_some() || size != 10 || data[0] & 2 != 0 {
                    return None;
                }
                let width = 1 + u32::from_le_bytes([data[4], data[5], data[6], 0]);
                let height = 1 + u32::from_le_bytes([data[7], data[8], data[9], 0]);
                ARTWORK.surface_bound(width, height)?;
                canvas = Some((width, height));
                None
            }
            b"VP8 " => {
                if data.len() < 10 || data[0] & 1 != 0 || &data[3..6] != b"\x9d\x01\x2a" {
                    return None;
                }
                Some((
                    u32::from(u16::from_le_bytes(data[6..8].try_into().ok()?) & 0x3fff),
                    u32::from(u16::from_le_bytes(data[8..10].try_into().ok()?) & 0x3fff),
                ))
            }
            b"VP8L" => {
                if data.len() < 5 || data[0] != 0x2f {
                    return None;
                }
                let bits = u32::from_le_bytes(data[1..5].try_into().ok()?);
                if bits >> 29 != 0 {
                    return None;
                }
                Some((1 + (bits & 0x3fff), 1 + ((bits >> 14) & 0x3fff)))
            }
            _ => None,
        };
        if let Some(dimensions) = dimensions {
            if image.is_some() || canvas.is_some_and(|canvas| canvas != dimensions) {
                return None;
            }
            ARTWORK.surface_bound(dimensions.0, dimensions.1)?;
            image = Some(dimensions);
        }
        offset = end.checked_add(size % 2)?;
        if offset > bytes.len() {
            return None;
        }
    }
    image
}

const DECODER_FLAG: &str = "--aurora-artwork-webp-decoder";
/// Narrow byte-only worker mode; it never initializes Tauri, opens an instance,
/// reads credentials or accepts paths/URLs. The OS enforces the allocation cap
/// because image-webp's advisory memory limit does not cover every allocation.
pub fn run_artwork_decoder_if_requested() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(DECODER_FLAG)) {
        return false;
    }
    if std::env::args_os().count() != 2 || !limit_decoder_memory() {
        std::process::exit(1);
    }
    use std::io::Read as _;
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take((ARTWORK.max_encoded_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        std::process::exit(1);
    }
    let Some(image) = decode_provider_image(&bytes) else {
        std::process::exit(1);
    };
    if std::io::stdout().write_all(image.bytes()).is_err() {
        std::process::exit(1);
    }
    true
}

#[cfg(windows)]
fn limit_decoder_memory() -> bool {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::Debug::{SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode},
            JobObjects::*,
            Threading::GetCurrentProcess,
        },
    };
    // All decoder allocations, including untrusted Huffman tables, are inside
    // this disposable process. Failure/OOM cannot abort the launcher.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return false;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        info.ProcessMemoryLimit = ARTWORK.max_decoded_bytes;
        let accepted = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        ) != 0
            && AssignProcessToJobObject(job, GetCurrentProcess()) != 0;
        CloseHandle(job);
        accepted
    }
}
#[cfg(target_os = "linux")]
fn limit_decoder_memory() -> bool {
    // RLIMIT_AS bounds future allocations after the executable's mappings;
    // RLIMIT_DATA additionally caps the worker's total heap to the same budget.
    let Some(pages) = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok())
    else {
        return false;
    };
    unsafe {
        let page_size = libc::sysconf(libc::_SC_PAGESIZE);
        if page_size <= 0 {
            return false;
        }
        let Some(maximum) = pages
            .checked_mul(page_size as u64)
            .and_then(|b| b.checked_add(ARTWORK.max_decoded_bytes as u64))
        else {
            return false;
        };
        libc::setrlimit(
            libc::RLIMIT_AS,
            &libc::rlimit {
                rlim_cur: maximum,
                rlim_max: maximum,
            },
        ) == 0
            && libc::setrlimit(
                libc::RLIMIT_DATA,
                &libc::rlimit {
                    rlim_cur: ARTWORK.max_decoded_bytes as u64,
                    rlim_max: ARTWORK.max_decoded_bytes as u64,
                },
            ) == 0
    }
}
#[cfg(not(any(windows, target_os = "linux")))]
fn limit_decoder_memory() -> bool {
    false
}

#[cfg(not(test))]
fn isolated_provider_image(bytes: &[u8]) -> Option<ValidatedPng> {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut command = Command::new(std::env::current_exe().ok()?);
    command
        .arg(DECODER_FLAG)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    let mut child = command.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take((ARTWORK.max_encoded_bytes + 1) as u64)
            .read_to_end(&mut output)
            .ok()?;
        Some(output)
    });
    let sent = child
        .stdin
        .take()
        .is_some_and(|mut input| input.write_all(bytes).is_ok());
    let deadline = Instant::now() + Duration::from_secs(10);
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break sent && status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let output = reader.join().ok().flatten()?;
    if !success {
        return None;
    }
    validate(&output, ARTWORK)
}

#[cfg(test)]
pub(crate) mod fixtures {
    pub(crate) fn gif(frames: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 2, 2, &[0, 0, 0, 255, 0, 0]).unwrap();
            for _ in 0..frames {
                let frame = gif::Frame {
                    width: 1,
                    height: 1,
                    left: 1,
                    top: 1,
                    buffer: std::borrow::Cow::Borrowed(&[1]),
                    transparent: Some(0),
                    ..Default::default()
                };
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }
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
    fn gif_snapshot_preserves_first_frame_offset_alpha_and_static_output() {
        for frames in [1, 3, 64] {
            let source = gif(frames);
            let image = validate_artwork(&source).unwrap();
            let mut reader = png::Decoder::new(Cursor::new(image.bytes()))
                .read_info()
                .unwrap();
            assert_eq!((reader.info().width, reader.info().height), (2, 2));
            assert!(reader.info().animation_control.is_none());
            let mut pixels = vec![0; reader.output_buffer_size()];
            reader.next_frame(&mut pixels).unwrap();
            reader.finish().unwrap();
            assert_eq!(&pixels[..12], &[0; 12]);
            assert_eq!(&pixels[12..], &[255, 0, 0, 255]);
            assert!(validate(&source, FAVICON).is_none());
        }
    }

    #[test]
    fn gif_rejects_truncation_oversized_canvas_frames_and_excessive_work() {
        let source = gif(3);
        for length in [6, 14, source.len() - 1] {
            assert!(validate_artwork(&source[..length]).is_none());
        }
        for width in [0u16, 1025, u16::MAX] {
            let mut image = source.clone();
            image[6..8].copy_from_slice(&width.to_le_bytes());
            assert!(validate_artwork(&image).is_none());
        }
        let mut outside = source.clone();
        let descriptor = outside.iter().position(|b| *b == 0x2c).unwrap();
        outside[descriptor + 1..descriptor + 3].copy_from_slice(&2u16.to_le_bytes());
        assert!(validate_artwork(&outside).is_none());
        assert!(validate_artwork(&gif(65)).is_none());
        // A valid trailer cannot disguise a damaged frame payload.
        let mut damaged = source.clone();
        damaged.truncate(damaged.len() - 5);
        damaged.push(0x3b);
        assert!(validate_artwork(&damaged).is_none());
        let mut excessive = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut excessive, 1024, 1024, &[0, 0, 0]).unwrap();
            let frame = gif::Frame {
                width: 1024,
                height: 1024,
                buffer: std::borrow::Cow::Owned(vec![0; 1024 * 1024]),
                ..Default::default()
            };
            for _ in 0..5 {
                encoder.write_frame(&frame).unwrap();
            }
        }
        assert!(validate_artwork(&excessive).is_none());
    }

    #[test]
    fn gif_accepts_complete_pixels_with_omitted_lzw_end_code() {
        let mut source = gif(1);
        let descriptor = source.iter().position(|b| *b == 0x2c).unwrap();
        let block_size = descriptor + 11;
        assert_eq!(&source[block_size..block_size + 3], &[2, 0x4c, 1]);
        // The one-pixel LZW stream contains clear + red; strip only its end
        // code, keeping the data-block terminator and final GIF trailer.
        source[block_size] = 1;
        source[block_size + 1] = 0x0c;
        source.remove(block_size + 2);
        assert!(validate_artwork(&source).is_some());
    }

    #[test]
    fn static_webp_decodes_to_persistable_png_and_animation_stays_rejected() {
        let lossy=Base64::decode_vec("UklGRjwAAABXRUJQVlA4IDAAAADQAQCdASoCAAIAAUAmJaACdLoB+AADsAD+8ut//NgVzXPv9//S4P0uD9Lg/9KQAAA=").unwrap();
        let normalized = validate_artwork(&lossy).unwrap();
        assert!(validate(normalized.bytes(), ARTWORK).is_some());
        assert!(validate(&lossy, FAVICON).is_none());
        let mut lossless = Vec::new();
        image_webp::WebPEncoder::new(&mut lossless)
            .encode(&[10, 20, 30, 127], 1, 1, image_webp::ColorType::Rgba8)
            .unwrap();
        let image = validate_artwork(&lossless).unwrap();
        let mut reader = png::Decoder::new(Cursor::new(image.bytes()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels).unwrap();
        assert_eq!(pixels, [10, 20, 30, 127]);
        let animated=Base64::decode_vec("UklGRsQAAABXRUJQVlA4WAoAAAACAAAAAQAAAQAAQU5JTQYAAAAAAAAAAABBTk1GSgAAAAAAAAAAAAEAAAEAADIAAAJWUDggMgAAADABAJ0BKgIAAgABQCYloAADcAD+8ut///mwP/bz/wR6Af//0uD//pcH//S4P/SkAAAAQU5NRkYAAAAAAAAAAAABAAABAAAyAAAAVlA4IC4AAAA0AQCdASoCAAIAAAAmJaAAA3AA/vtV4///S4P/+lwf/9Lg/9Lg//rV5Vesq6AA").unwrap();
        assert!(validate_artwork(&animated).is_none());
        assert!(validate_artwork(&lossless[..lossless.len() - 1]).is_none());
        let mut huge = lossy.clone();
        huge[26..28].copy_from_slice(&1025u16.to_le_bytes());
        assert!(validate_artwork(&huge).is_none());
        // A small extended canvas cannot disguise an oversized compressed image.
        let mut extended = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\0\0\0\0\x01\0\0\x01\0\0".to_vec();
        extended.extend_from_slice(&huge[12..]);
        let size = (extended.len() - 8) as u32;
        extended[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(validate_artwork(&extended).is_none());
    }

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
