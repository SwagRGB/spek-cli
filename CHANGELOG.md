# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-01-19

### Added
- **5 Color Palettes:** `audacity` (default), `magma`, `viridis`, `inferno`, `grayscale`.
- **Spectral Rolloff Indicator:** New `--rolloff` flag to visualize the 85% energy threshold.
- **High-Resolution Export:** Save spectrograms to PNG with `-s`.
- **Logarithmic Scale:** Option to use log frequency scale with `--log`.
- **Configuration System:** Auto-generates `~/.config/spek/config.toml` for persistent settings.
- **Professional UI:**
    - Nerd Font icons integration.
    - Clean title bar and axis labels.
    - dB scale legend.
- **Performance:** Optimized STFT processing with `rayon` parallelism.
- **Quality Analysis:** Improved spectrogram rendering for spotting lossy upscales.

### Changed
- Refactored entire codebase for modularity (`spectrogram.rs`, `decoder.rs`, `render.rs`).
- Updated `README.md` with visual examples and detailed documentation.
- Bumped dependencies to latest versions.

### Removed
- Legacy single-palette rendering.
