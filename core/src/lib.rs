pub mod appearance;
pub mod application;
pub mod artwork;
pub mod aurora;
pub mod auth;
pub mod cache;
pub mod config;
pub mod content_jobs;
pub mod content_recognition;
pub mod content_updates;
mod cosmetic_image;
#[cfg(all(debug_assertions, feature = "f3ra-desktop-acceptance"))]
mod desktop_acceptance;
mod import_source;
pub mod local_content;
pub mod local_pack;
pub mod mod_bisect;
pub mod pack_export;
pub mod storage;
pub mod world_backups;
// Bounded read-only NBT walking for saved-game discovery (servers.dat and
// level.dat). Raw spans also support the server editor and instance sync;
// those callers own their write-back semantics.
pub(crate) mod nbt;
pub mod saved_servers;
pub mod saved_worlds;
pub mod screenshots;

pub use cosmetic_image::run_artwork_decoder_if_requested;
pub mod cosmetics;
pub mod discord;
pub mod distribution;
pub mod downloads;
pub mod fabric;
pub mod forge;
pub mod game;
pub mod gameplay_history;
pub mod home_widgets;
pub mod host;
pub mod install;
mod installed_artwork;
pub mod instance_content;
pub mod instance_icons;
pub mod instance_mods;
pub mod instance_sync;
pub mod instances;
pub mod integrity;
pub mod launch;
pub mod minecraft;
pub mod mod_compatibility;
pub mod mod_icons;
pub mod modpacks;
pub mod modrinth;
pub mod mrpack;
pub mod neoforge;
pub mod pack_activation;
pub mod pack_merge;
pub mod pack_state;
pub mod pack_update;
pub mod paths;
mod performance;
pub mod pulsar;
pub mod runtime;
pub mod server_enrichment;
pub mod shortcuts;
pub mod transfer;
pub mod updates;

#[cfg(test)]
mod test_support;

/// Decompresses a raw DEFLATE stream produced at build time for bundled static tables.
pub fn inflate_raw(compressed: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(compressed).read_to_end(&mut out)?;
    Ok(out)
}
