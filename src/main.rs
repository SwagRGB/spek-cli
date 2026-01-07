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
    
    println!("Decoding {:?}...", args.file);
    let audio_data = decoder::decode_file(&args.file)
        .context("Failed to decode audio file")?;
        
    println!("Generating spectrogram ({}s, {}Hz)...", audio_data.duration_secs, audio_data.sample_rate);
    
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
    
    let viuer_conf = ViuerConfig {
        // We want to use the full width of the terminal
        width: Some(term_w as u32),
        height: Some(term_h as u32), // Leave some space for text?
        absolute_offset: false,
        transparent: false,
        ..Default::default()
    };
    
    // Print it
    viuer::print(&dynamic_img, &viuer_conf)?;
    
    Ok(())
}
