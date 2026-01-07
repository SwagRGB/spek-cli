pub mod config;
pub mod decoder;
pub mod spectrogram;
pub mod render;

use clap::Parser;
use std::path::PathBuf;
use anyhow::{Result, Context};
use viuer::Config as ViuerConfig;
use crossterm::terminal::size;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the audio file
    #[arg(required = true)]
    file: PathBuf,

    /// Width of the output image (optional, defaults to terminal width)
    #[arg(short, long)]
    width: Option<u32>,

    /// Height of the output image (optional, defaults to terminal height)
    #[arg(short = 'H', long)]
    height: Option<u32>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Load config
    let config = config::load_config().unwrap_or_else(|e| {
        eprintln!("Warning loading config: {}. Using defaults.", e);
        config::Config::default()
    });

    println!("═══════════════════════════════════════════════════════════");
    println!("  Spek-CLI - Audio Spectrum Analyzer");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Decode audio
    let audio_data = decoder::decode_file(&args.file)
        .context("Failed to decode audio file. Ensure it's a valid audio format (FLAC, MP3, WAV, ALAC, AAC).")?;

    println!();
    print_metadata(&args.file, &audio_data);
    println!();

    println!("Generating spectrogram...");

    // Determine dimensions
    let (term_w, term_h) = size().unwrap_or((80, 24));

    let img_width = args.width.unwrap_or(2048);
    let img_height = args.height.unwrap_or(1024);

    let mut spectrogram_img = spectrogram::generate_spectrogram(
        &audio_data.samples,
        audio_data.sample_rate,
        img_width,
        img_height,
        &config
    )?;

    render::draw_labels(&mut spectrogram_img, audio_data.sample_rate, audio_data.duration_secs, &config)?;

    let dynamic_img = image::DynamicImage::ImageRgb8(spectrogram_img);

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!();

    let viuer_conf = ViuerConfig {
        width: Some(term_w as u32),
        height: Some(term_h as u32),
        absolute_offset: false,
        transparent: false,
        ..Default::default()
    };

    // Print it
    viuer::print(&dynamic_img, &viuer_conf)?;

    Ok(())
}

fn print_metadata(file_path: &PathBuf, audio_data: &decoder::AudioData) {
    println!("┌─ File Information ─────────────────────────────────────┐");
    println!("│ File:       {:<42} │", truncate_path(file_path, 44));
    println!("├────────────────────────────────────────────────────────┤");
    println!("│ Codec:      {:<42} │", format_codec(&audio_data.metadata.codec));
    println!("│ Duration:   {:<42} │", format_duration(audio_data.duration_secs));
    println!("│ Sample Rate: {:<41} │", format!("{}Hz", audio_data.sample_rate));
    println!("│ Channels:   {:<42} │", format_channels(&audio_data.metadata.channel_layout));

    if let Some(bps) = audio_data.metadata.bits_per_sample {
        println!("│ Bit Depth:  {:<42} │", format!("{} bits", bps));
    }

    if let Some(br) = audio_data.metadata.bit_rate {
        println!("│ Bit Rate:   {:<42} │", format_bitrate(br));
    }

    println!("└────────────────────────────────────────────────────────┘");
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
    // Clean up codec string
    let codec = codec.replace("CodecType(", "").replace(")", "");

    match codec.as_str() {
        "Flac" => "FLAC (Free Lossless Audio Codec)".to_string(),
        "Mp3" => "MP3 (MPEG Audio Layer 3)".to_string(),
        "Aac" => "AAC (Advanced Audio Coding)".to_string(),
        "Alac" => "ALAC (Apple Lossless Audio Codec)".to_string(),
        "Vorbis" => "Vorbis (Ogg Vorbis)".to_string(),
        "Opus" => "Opus".to_string(),
        "Pcm" => "PCM (Uncompressed)".to_string(),
        _ => codec,
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

fn format_channels(layout: &str) -> String {
    // Clean up channel layout string
    let layout = layout.replace("Some(Channels(", "").replace("))", "");

    match layout.as_str() {
        "FRONT_LEFT | FRONT_RIGHT" => "Stereo (2.0)".to_string(),
        "FRONT_CENTRE" => "Mono (1.0)".to_string(),
        _ => layout,
    }
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
