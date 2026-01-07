use anyhow::Result;
use rustfft::{FftPlanner, num_complex::Complex};
use image::{RgbImage, Rgb};
use crate::config::{ColorStop, Config};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

pub fn generate_spectrogram(
    samples: &[f32],
    _sample_rate: u32,
    width: u32,
    height: u32,
    config: &Config,
) -> Result<RgbImage> {
    let window_size = 2048;
    let overlap = 0.75; // 75% overlap
    let hop_size = (window_size as f32 * (1.0 - overlap)) as usize;
    
    if samples.len() < window_size {
         return Err(anyhow::anyhow!("File too short (need at least {} samples)", window_size));
    }

    // Step 1: Compute STFT
    println!("Computing STFT...");
    let stft_result = compute_stft(samples, window_size, hop_size)?;
    
    // Step 2: Render to image
    println!("Rendering spectrogram...");
    let img = render_spectrogram(&stft_result, width, height, config)?;
    
    Ok(img)
}

struct StftResult {
    // 2D array: time_frames x frequency_bins
    magnitudes: Vec<Vec<f32>>,
    num_time_frames: usize,
    num_freq_bins: usize,
}

fn compute_stft(samples: &[f32], window_size: usize, hop_size: usize) -> Result<StftResult> {
    let num_time_frames = (samples.len() - window_size) / hop_size + 1;
    let num_freq_bins = window_size / 2;
    
    // Prepare window function (Hann)
    let window: Vec<f32> = (0..window_size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (window_size as f32 - 1.0)).cos()))
        .collect();

    // Setup progress bar
    let pb = ProgressBar::new(num_time_frames as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} frames ({percent}%, {eta})")
            .unwrap()
            .progress_chars("#>-")
    );
    pb.set_message("STFT");

    // Prepare FFT planner (thread-local for parallel processing)
    let magnitudes: Vec<Vec<f32>> = (0..num_time_frames)
        .into_par_iter()
        .map(|frame_idx| {
            let start = frame_idx * hop_size;
            let end = start + window_size;
            
            // Create FFT for this thread
            let mut planner = FftPlanner::new();
            let fft = planner.plan_fft_forward(window_size);
            
            // Apply window and prepare FFT input
            let mut buffer: Vec<Complex<f32>> = samples[start..end]
                .iter()
                .zip(window.iter())
                .map(|(&s, &w)| Complex { re: s * w, im: 0.0 })
                .collect();
            
            // Compute FFT
            fft.process(&mut buffer);
            
            // Extract magnitudes (only first half, up to Nyquist)
            let frame_mags: Vec<f32> = buffer[0..num_freq_bins]
                .iter()
                .map(|c| c.norm())
                .collect();
            
            // Update progress (note: this is approximate in parallel)
            if frame_idx % 10 == 0 {
                pb.inc(10);
            }
            
            frame_mags
        })
        .collect();

    pb.finish_with_message("STFT complete");

    Ok(StftResult {
        magnitudes,
        num_time_frames,
        num_freq_bins,
    })
}

fn render_spectrogram(
    stft: &StftResult,
    width: u32,
    height: u32,
    config: &Config,
) -> Result<RgbImage> {
    let mut img = RgbImage::new(width, height);
    let gradient = create_gradient_map(&config.colors.stops, 1024);
    
    // Setup progress bar
    let pb = ProgressBar::new(width as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} columns ({percent}%)")
            .unwrap()
            .progress_chars("#>-")
    );
    pb.set_message("Rendering");

    // Resample STFT to image dimensions
    for x in 0..width {
        // Map pixel x to time frame
        let time_pos = (x as f32 / width as f32) * stft.num_time_frames as f32;
        let frame_idx = time_pos.floor() as usize;
        let frame_idx = frame_idx.min(stft.num_time_frames - 1);
        
        let frame_mags = &stft.magnitudes[frame_idx];
        
        for y in 0..height {
            // y=0 is top (high freq), y=height-1 is bottom (low freq)
            let y_inverted = height - 1 - y;
            
            // Map pixel y to frequency bin
            let freq_pos = (y_inverted as f32 / height as f32) * stft.num_freq_bins as f32;
            
            // Use bilinear interpolation or max for better quality
            let bin_start = freq_pos.floor() as usize;
            let bin_end = (freq_pos.ceil() as usize).min(stft.num_freq_bins - 1);
            
            // Take max magnitude in range to avoid missing peaks
            let max_mag = if bin_start == bin_end {
                frame_mags[bin_start]
            } else {
                frame_mags[bin_start].max(frame_mags[bin_end])
            };
            
            // Convert to dB
            let normalized_mag = max_mag / (stft.num_freq_bins as f32 / 2.0);
            let db = 20.0 * (normalized_mag + 1e-9).log10();
            
            // Map dB to color
            let min_db = -100.0;
            let max_db = 0.0;
            let normalized_val = (db - min_db) / (max_db - min_db);
            let clamped = normalized_val.max(0.0).min(1.0);
            
            let color_idx = (clamped * 1023.0) as usize;
            let pixel = gradient[color_idx];
            
            img.put_pixel(x, y, pixel);
        }
        
        if x % 10 == 0 {
            pb.inc(10);
        }
    }

    pb.finish_with_message("Render complete");

    Ok(img)
}

fn create_gradient_map(stops: &[ColorStop], size: usize) -> Vec<Rgb<u8>> {
    let mut map = Vec::with_capacity(size);
    let mut sorted_stops = stops.to_vec();
    sorted_stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
    
    for i in 0..size {
        let pos = i as f32 / (size - 1) as f32;
        let mut start_stop = &sorted_stops[0];
        let mut end_stop = &sorted_stops[sorted_stops.len() - 1];
        
        for w in sorted_stops.windows(2) {
            if pos >= w[0].position && pos <= w[1].position {
                start_stop = &w[0];
                end_stop = &w[1];
                break;
            }
        }
        
        let range = end_stop.position - start_stop.position;
        let t = if range.abs() < f32::EPSILON { 0.0 } else { (pos - start_stop.position) / range };
        
        let start_color = hex_to_rgb(&start_stop.color);
        let end_color = hex_to_rgb(&end_stop.color);
        
        let r = (start_color[0] as f32 * (1.0 - t) + end_color[0] as f32 * t) as u8;
        let g = (start_color[1] as f32 * (1.0 - t) + end_color[1] as f32 * t) as u8;
        let b = (start_color[2] as f32 * (1.0 - t) + end_color[2] as f32 * t) as u8;
        
        map.push(Rgb([r, g, b]));
    }
    map
}

fn hex_to_rgb(hex: &str) -> [u8; 3] {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        [r, g, b]
    } else {
        [0, 0, 0]
    }
}
