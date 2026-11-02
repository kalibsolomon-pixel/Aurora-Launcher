# Oversized JPEG regression fixtures

These are the valid, solid-color JPEGs reproduced by the independent review:

- `solid-4096.jpg`: 4096 × 4096, 98,937 bytes (below both compressed-byte caps).
- `solid-8192.jpg`: 8192 × 8192, 394,181 bytes (below the artwork cap).

They were generated with Pillow as RGB solid-color images and are checked in
as encoded bytes so tests never allocate their large uncompressed surfaces.
Production now rejects JPEG entirely. The shared validator, acquisition/cache,
and server-status tests use these exact encoded probes; no internet is used.
