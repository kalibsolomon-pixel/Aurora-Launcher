use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

fn main() {
    pack_material_symbols();
    pack_sf_pro_fonts();

    #[cfg(windows)]
    {
        // The shared multi-resolution ICO is derived from assets/icon.png.
        // GPUI loads application icon resource 1 for its native window class.
        println!("cargo:rerun-if-changed=../native/assets/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../native/assets/icon.ico");
        resource.set("ProductName", "Aurora Launcher");
        resource.set("OriginalFilename", "aurora-launcher.exe");
        resource.set("InternalName", "aurora-launcher");
        resource.set("FileDescription", "Aurora Launcher");
        resource
            .compile()
            .expect("the Aurora application icon resource must compile");
    }
}

/// Compresses the Material Symbols table from `../native/src/symbols_data.rs`
/// into two raw-DEFLATE blobs (`symbols_paths.deflate` for icon rendering and
/// `symbols_search.deflate` for icon-picker tags/categories/popularity) so
/// release binaries stay compact and search metadata stays out of RAM until
/// the icon picker is opened.
fn pack_material_symbols() {
    let source_path = "../native/src/symbols_data.rs";
    println!("cargo:rerun-if-changed={source_path}");
    let src = fs::read_to_string(source_path).expect("symbols_data.rs must exist");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let mut categories: Vec<String> = Vec::new();
    let mut in_categories = false;
    let mut path_entries: Vec<(String, String)> = Vec::new();
    let mut search_entries: Vec<(String, u8, u32)> = Vec::new();

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub static CATEGORIES:") {
            in_categories = true;
            continue;
        }
        if in_categories {
            if trimmed == "];" {
                in_categories = false;
                continue;
            }
            if let Some((cat, _)) = parse_rust_str(trimmed) {
                categories.push(cat);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("    (") {
            let (name, rest) = parse_rust_str(rest).expect("symbol name");
            let rest = rest.strip_prefix(", ").expect("comma after name");
            let (tags, rest) = parse_rust_str(rest).expect("symbol tags");
            let rest = rest.strip_prefix(", ").expect("comma after tags");
            let (cat_str, rest) = rest.split_once(", ").expect("comma after category");
            let category: u8 = cat_str.parse().expect("category u8");
            let (pop_str, rest) = rest.split_once(", ").expect("comma after popularity");
            let popularity: u32 = pop_str.parse().expect("popularity u32");
            let (svg_path, _) = parse_rust_str(rest).expect("symbol path");
            path_entries.push((name, svg_path));
            search_entries.push((tags, category, popularity));
        }
    }

    assert!(
        path_entries.len() > 3000 && path_entries.len() == search_entries.len(),
        "unexpected Material Symbols entry count: {}",
        path_entries.len()
    );

    // Emit CATEGORIES constant directly so it remains a zero-cost &'static [&'static str].
    let mut cat_rs = String::from("pub static CATEGORIES: &[&str] = &[\n");
    for cat in &categories {
        cat_rs.push_str("    ");
        cat_rs.push_str(&format!("{cat:?}"));
        cat_rs.push_str(",\n");
    }
    cat_rs.push_str("];\n");
    fs::write(out_dir.join("symbols_categories.rs"), cat_rs).expect("write categories");

    // Pack paths blob: u32 count, then per entry: u16 name_len, name, u16 path_len, path.
    let mut raw_paths = Vec::with_capacity(2_400_000);
    raw_paths.extend_from_slice(&(path_entries.len() as u32).to_le_bytes());
    for (name, svg_path) in &path_entries {
        let nb = name.as_bytes();
        let pb = svg_path.as_bytes();
        raw_paths.extend_from_slice(&(u16::try_from(nb.len()).expect("name len")).to_le_bytes());
        raw_paths.extend_from_slice(nb);
        raw_paths.extend_from_slice(&(u16::try_from(pb.len()).expect("path len")).to_le_bytes());
        raw_paths.extend_from_slice(pb);
    }
    write_deflated(&out_dir.join("symbols_paths.deflate"), &raw_paths);

    // Pack search blob: u32 count, then per entry: u8 category, u32 popularity, u16 tags_len, tags.
    let mut raw_search = Vec::with_capacity(1_400_000);
    raw_search.extend_from_slice(&(search_entries.len() as u32).to_le_bytes());
    for (tags, category, popularity) in &search_entries {
        let tb = tags.as_bytes();
        raw_search.push(*category);
        raw_search.extend_from_slice(&popularity.to_le_bytes());
        raw_search.extend_from_slice(&(u16::try_from(tb.len()).expect("tags len")).to_le_bytes());
        raw_search.extend_from_slice(tb);
    }
    write_deflated(&out_dir.join("symbols_search.deflate"), &raw_search);
}

fn write_deflated(path: &std::path::Path, raw: &[u8]) {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(raw).expect("deflate write");
    let compressed = encoder.finish().expect("deflate finish");
    fs::write(path, compressed).expect("write deflated symbols blob");
}

/// Compresses the subsetted SF Pro Text font weights (Regular, Medium,
/// Semibold, Bold) into a single raw-DEFLATE blob (`sf_pro_fonts.deflate`)
/// so the binary stays small while shipping Apple's SF Pro typography.
fn pack_sf_pro_fonts() {
    const WEIGHTS: [&str; 4] = [
        "assets/fonts/SF-Pro-Text-Regular.otf",
        "assets/fonts/SF-Pro-Text-Medium.otf",
        "assets/fonts/SF-Pro-Text-Semibold.otf",
        "assets/fonts/SF-Pro-Text-Bold.otf",
    ];
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let mut raw = Vec::with_capacity(800_000);
    raw.extend_from_slice(&(WEIGHTS.len() as u32).to_le_bytes());
    for rel in WEIGHTS {
        println!("cargo:rerun-if-changed={rel}");
        let bytes = fs::read(rel).unwrap_or_else(|e| panic!("failed to read {rel}: {e}"));
        raw.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        raw.extend_from_slice(&bytes);
    }
    write_deflated(&out_dir.join("sf_pro_fonts.deflate"), &raw);
}

fn parse_rust_str(input: &str) -> Option<(String, &str)> {
    let rest = input.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '"' => return Some((out, &rest[idx + 1..])),
            '\\' => {
                let (_, escaped) = chars.next()?;
                match escaped {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
    }
    None
}
