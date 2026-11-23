# Aurora Launcher 2.7.1

**Windows Defender false-positive mitigations.** Reduces static machine-learning
false-positive triggers (`Trojan:Win32/Bearfoos!ml`) on unsigned Windows builds:

- Embeds bundled SF Pro Text fonts and Material Symbols tables uncompressed in
  the executable instead of high-entropy raw-DEFLATE blobs (also avoiding
  startup decompression).
- Completes Windows PE version metadata (`CompanyName` and `LegalCopyright`) on
  `aurora-launcher.exe`.
- Switches the Windows release profile from `opt-level = "s"` + `strip = true`
  + single-unit full LTO to `opt-level = 3`, `lto = "thin"`, and `strip = false`.
- Switches the NSIS installer compressor from `/SOLID lzma` to standard `zlib`.

Update trust, application identity and managed-data locations remain unchanged.
Windows publisher signing remains unconfigured; signed update artifacts provide
release authentication.
