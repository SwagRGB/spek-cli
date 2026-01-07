# spek-cli

A terminal-based acoustic spectrum analyzer (spectrogram) viewer, written in Rust. Designed for checking audio quality (identifying "fake" lossless files) directly from your terminal.

![Spectrogram Example](https://upload.wikimedia.org/wikipedia/commons/c/c5/Spectrogram-19thC.png)

## Features

*   **High-Resolution Spectrograms**: Uses proper FFT (not blocky ASCII) to visualize audio frequencies.
*   **Terminal Graphics**: Supports high-quality image rendering in compatible terminals (Kitty, iTerm2, Sixel) via `viuer`.
*   **Broad Format Support**: Supports FLAC, ALAC, WAV, MP3, and more (powered by `symphonia`).
*   **Customizable**: Configurable color palettes and fonts.
*   **Performance**: Fast processing using Rust.

## Installation

### Arch Linux

1.  **Install Rust**:
    ```bash
    sudo pacman -S rust
    ```

2.  **Clone and Build**:
    ```bash
    git clone <repository-url>
    cd spek-cli
    cargo build --release
    ```

3.  **Install (Optional)**:
    You can copy the binary to your path:
    ```bash
    sudo cp target/release/spek-cli /usr/local/bin/spek
    ```

### Other Distributions

Ensure you have `cargo` installed (usually via `rustup` or your package manager), then build from source as shown above.

## Usage

Basic usage:
```bash
spek-cli path/to/audio.flac
```

Options:
*   `-w, --width <WIDTH>`: Force a specific image width in pixels (default: 2048 or auto).
*   `-H, --height <HEIGHT>`: Force a specific image height in pixels (default: 1024 or auto).

Example:
```bash
spek -w 3000 -H 1000 music.flac
```

## Configuration

`spek-cli` looks for a configuration file at `~/.config/spek/config.toml`.

### Example Config

```toml
# ~/.config/spek/config.toml

# Path to a custom font for axis labels (optional).
# If not set, spek-cli tries to find a system default using `fc-match`.
# font_path = "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf"

[colors]
# Define the color gradient for the spectrogram (0.0 to 1.0)
stops = [
    { position = 0.0, color = "#000000" }, # Silence (Black)
    { position = 0.4, color = "#0000FF" }, # Low intensity (Blue)
    { position = 0.7, color = "#FF0000" }, # High intensity (Red)
    { position = 1.0, color = "#FFFFFF" }  # Max intensity (White)
]
```

## Terminal Support

For the best experience, use a terminal emulator that supports the **Kitty Graphics Protocol** (like [Kitty](https://sw.kovidgoyal.net/kitty/)) or **Sixel** (like [Alacritty](https://github.com/alacritty/alacritty) *with sixel patches* or `mlterm`).

If your terminal does not support these, `spek-cli` will fallback to block-character rendering, which is functional but less detailed.
