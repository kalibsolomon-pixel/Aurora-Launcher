# Launcher correction source review

This is a source correction pass on `eb32c0fd1b6c2614d567b88477742de4f40b4e22`, not a 1.4.1 release. Package/native versions remain 1.4.0. No production publication or authority mutation is authorized.

## Investigation before implementation

- Installed Mods, Resource Packs and Shaders already share `InstalledArtwork` and the Rust project-identity cache. Cached raster bytes become data URLs. Production CSP admits the Modrinth CDN but omits `data:` from `img-src`, so all three installed surfaces fail together. Browse already admits only Rust-normalized official CDN artwork and handles image errors; it does not share this CSP failure. Keep that provider architecture and unify the deliberate fallback presentation.
- Recent Servers receives actual status-protocol favicons, not website banners. Native normalization and serialization retain the data URL, including persisted offline reuse. The installed 1.4.0 app visibly reproduces broken images for `mcpvp.club`, `minemen.club` and `pvphq.com`. The same CSP omission blocks them, and the widget has no image-error fallback. Cached favicons also need validation on read before frontend exposure. No new favicon websites, scraping or banner provider is needed.
- The Home listbox already owns correct native selection, arrows/Home/End, typeahead and focus return. Its trigger/menu use older small radii and strong menu borders; the caret is a Unicode character. Restyle this existing component with Phase-F tokens and the existing local SVG chevron.
- Instances combines the global group's bottom margin with a separate grid gap; the Modpacks-to-columns gap differs from the column/card gap. Use one outer gap and remove outer group margins within this screen. Preserve existing-first DOM order and responsive stacking.
- Settings currently exposes Appearance, Home, Updates and Discord & privacy. Windows shortcut management remains implemented in Rust and its frontend store but disappeared from normal Settings during Phase F. General will contain that existing desktop-integration behavior: live Desktop shortcut status/action, installer-owned Start menu status, and Refresh status. No new config fields, sync, startup policy, Java settings or invented launch behavior.
- Discord already runs in a detached native actor. It tries enabled connections on boot, but repeats at a fixed 15-second interval, exposes manual Connect/Reconnect, and keeps publishing null activity while disabled after connection. Add capped retry backoff and a disabled clear/disconnect transition; retain all privacy opt-ins and narrow commands.
- Accounts uses a native `<dialog>` with a centered 680px maximum width. Keep native modal focus/inert/Escape semantics and existing account contents/actions; change geometry to a restrained full-height left drawer beside the left account dock, with internal scrolling and backdrop dismissal.
- Create-instance help hard-codes Aurora 2.1.2 / Fabric 0.19.5, while the existing bundled production release source already contains Client 2.1.5. Derive compatible help from the native compatibility DTO instead of introducing another version constant. Historical acceptance documents describe their original releases and are not current UI copy.

## Evidence and safety baseline

Full status, tracked paths, protected untracked SHA-256s and critical-file hashes are recorded under `C:\Users\kalib\AppData\Local\Temp\aurora-correction-f66955a11232483ebf30702bbd44f08e`. HEAD, main and origin/main all match the baseline; main/origin divergence is 0/0. Original branch: `codex/ref-backed-update-authority`; correction branch: `codex/launcher-correction-source`. No tracked modifications existed. All existing diagnostics are protected.

Implementation, checks, screenshot evidence and acceptance results will be recorded here as they complete.
