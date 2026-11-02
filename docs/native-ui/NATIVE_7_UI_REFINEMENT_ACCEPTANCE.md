# NATIVE-7 preview/layout refinement — retained continuation record

The NATIVE-8 takeover found these changes already implemented and uncommitted.
They are preserved with the native UX work. This does not change the historical
NATIVE-7 partial-acceptance classification or imply native release parity.

The preview uses immutable scene snapshots and one blocking CPU render in flight,
with one implicit latest pose. UI-thread input does not rasterize or enqueue an
unbounded frame backlog. Scene/epoch checks prevent stale account/local imagery
from replacing the current preview. The cape outward face and local orientation
are corrected; asymmetric synthetic texture tests cover lining and HD scaling.
Horizontal drag follows the corrected native direction; vertical bounds remain.

Existing evidence: `docs/native-ui/native7-refinement/` contains standalone 600-frame raster samples. Original account-bearing
screenshots are preserved locally and ignored; they are not published. Classic mean: 2.4276 → 2.0080 ms;
slim: 2.2876 → 1.8615 ms. Those are retained CPU raster observations, not physical
presentation or current end-to-end interaction latency. No live cape/profile
mutation was performed in continuation. `preview_probe` remains opt-in under
`disposable-data-root` and uses synthetic pixels, skipping account/provider work.

See [docs/native-ui/NATIVE_8_ACCEPTANCE.md](NATIVE_8_ACCEPTANCE.md) for the current build, regression
results, real native window evidence, preservation and remaining limitations.
