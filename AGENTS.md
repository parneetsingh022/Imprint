# Imprint — Agent Instructions

## Project Overview
Imprint is a Rust desktop application for preparing passport and ID photos for printing. Its core workflows include importing images, cropping and resizing them to precise physical dimensions, arranging multiple copies in grids on printable sheets, and exporting print-ready layouts.

## Engineering Principles
- Follow the existing project structure and conventions before introducing new patterns.
- Prefer idiomatic, readable, maintainable Rust and small, focused modules.
- Keep UI, image processing, print-layout calculations, and export logic separate.
- Avoid unrelated refactors and unnecessary dependencies.
- Ask before making significant architectural or dependency changes.

## Rust Guidelines
- Use `Result` and descriptive errors for recoverable failures.
- Avoid `unwrap()` and `expect()` on user-controlled input or other fallible production paths.
- Avoid `unsafe` unless clearly necessary and documented.
- Prefer borrowing over unnecessary cloning and allocation.
- Format code with `cargo fmt` and check with `cargo clippy`.

## Image Processing
- Preserve the original image; perform edits on derived representations.
- Distinguish source-image coordinates, preview coordinates, and output pixel coordinates.
- Preserve aspect ratio unless the user explicitly requests distortion.
- Handle image orientation and metadata intentionally.
- Use suitable high-quality resampling for final output.
- Avoid repeated resampling that reduces image quality.

## Print Accuracy and Layout
- Represent physical measurements explicitly (e.g., millimeters, inches, DPI/PPI, pixels).
- Base print geometry on physical dimensions, not preview-window dimensions.
- Convert millimeters to output pixels using `pixels = millimeters / 25.4 * dpi`.
- Define a consistent rounding policy and test it to avoid cumulative placement errors.
- Make margins, gutters, rows, columns, and photo dimensions configurable.
- Validate that all images fit within the selected sheet; report overflow rather than silently changing dimensions.
- Verify exported pixel dimensions and appropriate resolution metadata where the format supports it.
- Do not assume passport-photo dimensions are universal; use explicit presets or user-specified sizes.

## Testing and Validation
- Add unit tests for unit conversions, crop geometry, grid placement, and boundary cases.
- Test portrait and landscape inputs, unusual aspect ratios, and images too small for the requested output.
- Test edge cases such as zero or negative dimensions, invalid DPI, and layouts that exceed sheet bounds.
- Add regression tests for bug fixes.
- When applicable, run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
- Report any checks that could not be run; do not claim tests passed unless they were executed.

## Agent Workflow
1. Read the relevant source files and existing tests.
2. Explain the intended approach before substantial changes.
3. Implement the smallest coherent solution.
4. Add or update tests for behavior changes.
5. Run applicable formatting, linting, and tests.
6. Summarize changed files, test results, and remaining limitations.

## Git and Pull Requests
- Keep changes focused and commit messages descriptive.
- Do not commit binaries, build artifacts, secrets, or temporary files.
- Do not force-push, rewrite history, commit, push, or create a pull request without explicit authorization.

## Technology Stack

Imprint is a native, cross-platform desktop application written in Rust.

### Approved Frameworks and Libraries

- **GUI:** `egui` and `eframe`
- **Image processing:** `image`
- **Advanced image operations:** `imageproc`
- **File dialogs:** `rfd`
- **Serialization:** `serde` and `serde_json`
- **PDF export:** `printpdf`
- **Error handling:** `thiserror`
- **Logging:** `tracing`

### Framework Rules

- Use Rust for application logic, image processing, and the user interface.
- Use `egui` for all user-interface components.
- Use `eframe` for native window management and application lifecycle.
- Use `egui`'s painting APIs for interactive photo grids, selection outlines, and crop overlays.
- Use the `image` crate for raster image processing and exporting.
- Keep image-processing logic independent of the GUI framework.
- Keep layout calculations independent of rendering and exporting.
- Use `printpdf` for PDF generation when PDF export is required.
- Avoid introducing alternative GUI frameworks such as `iced`, `Slint`, `GTK`, or `Tauri` without explicit approval.
- Do not add dependencies that duplicate functionality already provided by approved crates.
- Check existing dependencies and their versions before introducing new ones.
- Prefer stable Rust and actively maintained libraries.

### Rendering and Export

- Render interactive previews using `egui`.
- Generate final print-ready output from the underlying image data, not from screenshots of the GUI.
- Use physical dimensions and DPI to determine output pixel dimensions.
- Keep preview rendering separate from high-resolution export.
- Preserve original image quality whenever possible.