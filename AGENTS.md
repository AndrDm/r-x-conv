# AGENTS.md

Small single-crate egui/eframe app that converts TIFF images to DICONDE (DICOM flavor used in NDT/industrial imaging). All app logic lives in `src/main.rs`; no tests, no CI, no task runner.

## Build & run

- Desktop (Windows MSVC): `cargo run`.
- Web: `cargo build --target wasm32-unknown-unknown` (target is installed via rustup). `index.html` at the repo root is the host page; serving/codegen tooling (e.g. Trunk) is **not** committed. Verify wasm compiles after changes: `cargo build --target wasm32-unknown-unknown`.
- No `cargo clippy` config; effective verification is `cargo build` (both targets) + `cargo fmt`.

## Formatting

- `rustfmt.toml` is non-default: **hard tabs** (not 4 spaces), `max_width = 100`, `use_small_heuristics = "Max"`. Run `cargo fmt` instead of hand-formatting; the existing code is already in this style.

## Editing src/main.rs

Most logic is **duplicated per target** and each copy only compiles on its own target:

- `open_tiff`, `save_dicom`, and `main` each exist in two versions guarded by `#[cfg(not(target_arch = "wasm32"))]` / `#[cfg(target_arch = "wasm32")]`. The DICOM element construction in `save_dicom` is duplicated in both copies — change both.
- Any tag/VR/metadata change must also take into account that the wasm path serializes to bytes and triggers a browser download, while the native path writes the file with `write_to_file`.
- `App` also has cfg-gated fields (wasm-only `pending_image`); don't "clean up" cfg attributes.
- Web entry panics unless `index.html` contains `<canvas id="the_canvas_id">` (renaming the id breaks the wasm build), which is the only content the wasm build needs from the page.

## Domain notes

- Conversion: image → 16-bit grayscale (`to_luma16`), single frame, 1 sample, `MONOCHROME2`, uncompressed Explicit VR Little Endian (`transfer_syntax "1.2.840.10008.1.2.1"`), pixel data in VR `OW`.
- UI label "Component Name (0010,0010)" is the DICOM *Patient Name* tag; the code uses standard DICOM tags, not separate DICONDE tags.
- Pixel spacing (0028,0030) is written as one value repeated for both row/column components.
- Errors are not surfaced to the user: native saves print via `eprintln!` (invisible in the GUI); wasm logs to the browser console.
