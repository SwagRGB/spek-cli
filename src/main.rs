pub mod config;
pub mod decoder;
pub mod spectrogram;
pub mod render;


use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::time::Instant;
use anyhow::{Result, Context};
use viuer::Config as ViuerConfig;
use crossterm::terminal::size;
use owo_colors::OwoColorize;

#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq)]
pub enum Palette {
    #[default]
    Audacity,
    Magma,
    Viridis,
    Inferno,
    Grayscale,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Audio Spectrum Analyzer - Check audio quality from your terminal", long_about = None)]
struct Args {
    /// Path to the audio file
    #[arg(required = true)]
    file: PathBuf,

    /// Width of the output image in pixels
    #[arg(short, long)]
    width: Option<u32>,

    /// Height of the output image in pixels
    #[arg(short = 'H', long)]
    height: Option<u32>,

    /// Use logarithmic frequency scale (better for music analysis)
    #[arg(long)]
    log: Option<bool>,

    /// Color palette for the spectrogram
    #[arg(short = 'p', long, value_enum)]
    palette: Option<Palette>,

    /// Quiet mode (suppress all progress output)
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Save spectrogram to PNG file instead of displaying in terminal
    #[arg(short = 's', long)]
    save: Option<PathBuf>,

    /// Show timing statistics after processing
    #[arg(short = 'v', long)]
    verbose: Option<bool>,

    /// Show spectral rolloff indicator line. The rolloff frequency is where
    /// 85% of the audio energy is concentrated. Useful for detecting lossy
    /// compression - MP3s typically show a steep rolloff around 16kHz.
    #[arg(long)]
    rolloff: Option<bool>,


}

fn main() -> Result<()> {
    let args = Args::parse();
    let total_start = Instant::now();

    // Load config (creates default if doesn't exist)
    let mut config = config::load_config().unwrap_or_else(|e| {
        if !args.quiet {
            eprintln!("{} {}", "".yellow(), format!("Config warning: {}. Using defaults.", e).dimmed());
        }
        config::Config::default()
    });

    // Merge CLI args with config defaults (CLI takes priority)
    let use_log = args.log.unwrap_or(config.defaults.log_scale);
    let use_rolloff = args.rolloff.unwrap_or(config.defaults.rolloff);
    let use_verbose = args.verbose.unwrap_or(config.defaults.verbose);
    let width = args.width.unwrap_or(config.defaults.width);
    let height = args.height.unwrap_or(config.defaults.height);
    
    // Handle palette: CLI > config > default
    let palette = args.palette.unwrap_or_else(|| config::parse_palette(&config.defaults.palette));
    
    // Apply palette
    config.colors.stops = config::get_palette_stops(palette);

    if !args.quiet {
        print_header();
    }



    // Decode audio
    let decode_start = Instant::now();
    let audio_data = decoder::decode_file(&args.file, args.quiet)
        .context("Failed to decode audio file. Ensure it's a valid audio format (FLAC, MP3, WAV, ALAC, AAC).")?;
    let decode_time = decode_start.elapsed();

    if !args.quiet {
        println!();
        print_metadata(&args.file, &audio_data);
        println!();
        println!("{}", "Generating spectrogram...".cyan());
    }

    // Determine dimensions
    let (term_w, term_h) = size().unwrap_or((80, 24));
    
    let stft_start = Instant::now();
    let spectrogram_result = spectrogram::generate_spectrogram(
        &audio_data.samples,
        audio_data.sample_rate,
        width,
        height,
        &config,
        !use_log,  // linear = !log
        args.quiet,
        use_rolloff,
    )?;
    let stft_time = stft_start.elapsed();

    let render_start = Instant::now();
    let render_options = render::RenderOptions {
        linear: !use_log,
        show_rolloff: use_rolloff,
        rolloff_frequencies: spectrogram_result.rolloff_frequencies,
    };
    let final_img = render::prepare_final_image(
        spectrogram_result.image, 
        audio_data.sample_rate, 
        audio_data.duration_secs, 
        &config, 
        render_options,
    )?;
    let render_time = render_start.elapsed();



    let dynamic_img = image::DynamicImage::ImageRgb8(final_img);

    // Handle save option
    if let Some(ref save_path) = args.save {
        dynamic_img.save(save_path)
            .with_context(|| format!("Failed to save image to {:?}", save_path))?;
        if !args.quiet {
            println!();
            println!("{} Saved to {}", "".green().bold(), save_path.display().to_string().cyan());
        }
    } else {
        if !args.quiet {
            println!();
            print_separator();
            println!();
        }

        let viuer_conf = ViuerConfig {
            width: Some(term_w as u32),
            height: Some(term_h as u32),
            absolute_offset: false,
            transparent: false,
            ..Default::default()
        };

        viuer::print(&dynamic_img, &viuer_conf)?;
    }

    // Print timing statistics if verbose
    if use_verbose {
        let total_time = total_start.elapsed();
        println!();
        println!("{}", " Timing Statistics".bright_magenta().bold());
        println!("  {} {:>8.2?}", "Decoding:".dimmed(), decode_time);
        println!("  {} {:>8.2?}", "STFT:    ".dimmed(), stft_time);
        println!("  {} {:>8.2?}", "Render:  ".dimmed(), render_time);
        println!("  {} {:>8.2?}", "Total:   ".bright_white().bold(), total_time);
    }

    Ok(())
}

fn print_header() {
    println!();
    println!("{}", "───────────────────────────────────────────────────────".bright_blue());
    println!("   {}", " Spek-CLI  Audio Spectrum Analyzer".bright_white().bold());
    println!("{}", "───────────────────────────────────────────────────────".bright_blue());
    println!();
}

fn print_separator() {
    println!("{}", "═══════════════════════════════════════════════════════════".bright_blue().dimmed());
}

fn print_metadata(file_path: &PathBuf, audio_data: &decoder::AudioData) {
    println!("{}", "┌─ File Information ─────────────────────────────────────┐".bright_blue());
    print_row("File", &truncate_path(file_path, 42));
    println!("{}", "├────────────────────────────────────────────────────────┤".bright_blue());
    print_row("Codec", &format_codec(&audio_data.metadata.codec));
    print_row("Duration", &format_duration(audio_data.duration_secs));
    print_row("Sample Rate", &format!("{}Hz", audio_data.sample_rate));
    print_row("Channels", format_channels(&audio_data.metadata.channel_layout));

    if let Some(bps) = audio_data.metadata.bits_per_sample {
        print_row("Bit Depth", &format!("{} bits", bps));
    }

    if let Some(br) = audio_data.metadata.bit_rate {
        print_row("Bit Rate", &format_bitrate(br));
    }

    println!("{}", "└────────────────────────────────────────────────────────┘".bright_blue());
}

fn print_row(label: &str, value: &str) {
    use owo_colors::OwoColorize;
    // Label column is 14 chars, value column fills the rest (40 chars)
    println!("{} {:<14} {:<40}{}", 
        "│".bright_blue(), 
        format!("{}:", label).cyan(),
        value,
        "│".bright_blue()
    );
}

fn truncate_path(path: &PathBuf, max_len: usize) -> String {
    let path_str = path.display().to_string();
    if path_str.len() <= max_len {
        path_str
    } else {
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        if filename.len() > max_len - 3 {
            format!("...{}", &filename[filename.len()-(max_len-3)..])
        } else {
            format!("...{}", filename)
        }
    }
}

fn format_codec(codec: &str) -> String {
    match codec {
        "FLAC" => "FLAC (Free Lossless Audio Codec)".to_string(),
        "MP3" => "MP3 (MPEG Audio Layer 3)".to_string(),
        "AAC" => "AAC (Advanced Audio Coding)".to_string(),
        "ALAC" => "ALAC (Apple Lossless Audio Codec)".to_string(),
        "Vorbis" => "Vorbis (Ogg Vorbis)".to_string(),
        _ => codec.to_string(),
    }
}

fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;
    let millis = ((seconds - total_seconds as f64) * 1000.0) as u64;

    if hours > 0 {
        format!("{}:{:02}:{:02}.{:03}", hours, minutes, secs, millis)
    } else {
        format!("{}:{:02}.{:03}", minutes, secs, millis)
    }
}

fn format_channels(layout: &str) -> &str {
    layout
}

fn format_bitrate(bitrate: u64) -> String {
    let kbps = bitrate / 1000;
    if kbps > 1000 {
        let mbps = kbps as f64 / 1000.0;
        format!("{:.2} Mbps", mbps)
    } else {
        format!("{} kbps", kbps)
    }
}


