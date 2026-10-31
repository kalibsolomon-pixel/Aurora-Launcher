# NATIVE-0 framework decision

**Selected: Slint 1.18 (Rust API + `.slint` markup), winit backend, software renderer.**

Measurements: [`BENCHMARKS.md`](BENCHMARKS.md). Prototype sources:
[`native/prototypes/slint-proto`](../../native/prototypes/slint-proto) and
[`native/prototypes/gpui-proto`](../../native/prototypes/gpui-proto).

## Candidates and current releases (checked 2026-10-31 on crates.io)

| Framework | Version | License | Renderer(s) on Windows | Accessibility |
| --- | --- | --- | --- | --- |
| Slint | 1.18.1 | GPL-3.0 **or** Slint Royalty-free 2.0 **or** commercial | software, FemtoVG (OpenGL), Skia | AccessKit (UI Automation on Windows, AT-SPI on Linux) — verified below |
| GPUI | 0.2.2 (Zed) | Apache-2.0 | DirectX 11 (Windows), Vulkan/Blade (Linux) | not established for standalone use |
| Iced | 0.14.0 | MIT | wgpu, tiny-skia | not evaluated (see below) |
| egui / eframe | 0.37.0 | MIT / Apache-2.0 | glow/wgpu | AccessKit |

## Criteria

| Criterion (weight) | Slint (software) | GPUI | Notes |
| --- | --- | --- | --- |
| Idle memory (critical) | 86 MiB private, 0 GPU | 70 MiB private + 34 MiB GPU | Tauri: 206 MiB private + 72 MiB GPU, 7 processes |
| Idle CPU/GPU (critical) | 0.36 % idle, 0.29 % minimized, no GPU | 0.94 % idle, **2.6 % minimized** | GPUI keeps scheduling frames while minimized |
| Rust integration (critical) | generated Rust types, globals, callbacks; drives the existing Tokio core | Rust-first entity model | both fine |
| Maintainability (critical) | stable 1.x API since 2023, declarative markup, live preview tooling | pre-1.0, API churn, forks (`gpui-ce`, `gpui-pre`) | |
| Windows compatibility (critical) | works on this host; per-monitor DPI correct at 125 % | works | |
| Accessibility/keyboard (high) | UIA tree exposes roles, names, values, invoke/toggle — verified with Windows UI Automation | no verified support | |
| Visual customization (high) | full custom components; shared tokens | full | |
| Startup (high) | 26 ms first window, ~680 ms quiescent | 167 ms / ~830 ms | |
| Linux portability (high) | winit X11/Wayland, software renderer needs no GPU stack | Vulkan required | |
| Dependency maturity (high) | single vendor, commercial backing | Zed-internal priority | |
| Widgets | text input, slider, combo, scroll, popup, AboutSlint | no text field, slider or dropdown in GPUI itself | GPUI would need a component library or hand-built text editing |
| License (gate) | **passes** under Royalty-free 2.0 with attribution | passes | see below |

## Why Slint

1. Lowest committed memory with **zero GPU memory**, flat idle CPU, near-zero
   minimized CPU, and the fastest first window.
2. Event-driven redraw: a static screen does not repaint. Animations are
   bounded (control transitions ≤150 ms, disabled by reduced motion).
3. Real platform accessibility through AccessKit — the native launcher's
   tabs, buttons, switches, sliders, combo boxes, lists and text fields
   appear in Windows UI Automation with names and states, and are operable
   through Invoke/Toggle patterns (this is how acceptance drove the app).
4. A stable API with first-class Rust integration; the UI contract lives in
   typed globals that Rust controllers bind to, keeping presentation separate
   from launcher authority.

## Why not the others

* **Slint FemtoVG (OpenGL):** the AMD OpenGL driver committed ~391 MiB private
  and ~232 MiB GPU memory — more than the WebView. The renderer stays
  available behind the `renderer-femtovg` feature/`SLINT_BACKEND` for
  comparison only.
* **GPUI:** competitive memory, but continuous frame scheduling (≈2.6 % CPU while
  minimized in all runs), no built-in text input/slider/dropdown, pre-1.0
  API, and unverified accessibility. The memory advantage (~16 MiB private)
  does not outweigh idle CPU and the work of building text editing.
* **Iced / egui:** not prototyped. The task's decision rule was to evaluate
  them only if Slint and GPUI were both unsuitable; Slint met every gate.
  egui's reactive mode and AccessKit support make it a credible fallback,
  but its immediate-mode styling makes the Spxctra component contract harder
  to enforce. Iced's wgpu path would carry the GPU-driver costs observed with
  FemtoVG; its tiny-skia path was not measured.

## Software vs GPU rendering

On this machine GPU acceleration costs far more memory than it saves CPU: a
1400×950 software frame redraws only damaged regions, and an idle launcher
draws nothing. The renderer can be switched per run with `SLINT_BACKEND`
(e.g. `winit-femtovg`) for diagnostics. Re-measure on low-end and Linux
hosts before changing the default.

## Licensing

Slint is triple-licensed. Aurora uses the **Slint Royalty-free License 2.0**,
which covers desktop applications on the condition of attribution: either
the `AboutSlint` widget in an About screen reachable from the top-level
navigation, or a Slint badge on the public download page. The native
launcher shows `AboutSlint` in **Settings → About** with a one-line license
statement. The license excludes embedded systems (not applicable) and
forbids redistributing Slint's API itself (not done). Aurora's own code stays
MIT. If the owner prefers the badge route instead, the About widget can be
removed in the same change that adds the badge to the website. GPL-3.0 is
not used.

Third-party notices for the native host are in
[`native/THIRD_PARTY_NOTICES.md`](../../native/THIRD_PARTY_NOTICES.md).

## Decision stability

Revisit only with material new evidence: a measured regression on
low-end/Linux hardware, a Slint licensing change, or an accessibility gap
that blocks a required workflow.
