use anyhow::Result;
use rustfft::{FftPlanner, num_complex::Complex};
use image::{RgbImage, Rgb};
use crate::config::{ColorStop, Config};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::sync::Arc;

/// Result containing spectrogram image, optional rolloff data, and STFT for quality analysis
pub struct SpectrogramResult {
    pub image: RgbImage,
    pub rolloff_frequencies: Option<Vec<f32>>, // Hz per time frame
    pub stft: StftResult, // For quality analysis
}

pub fn generate_spectrogram(
    samples: &[f32],
    sample_rate: u32,
    width: u32,
    height: u32,
    config: &Config,
    linear: bool,
    quiet: bool,
    compute_rolloff: bool,
) -> Result<SpectrogramResult> {
    let window_size = 2048;
    let overlap = 0.75; // 75% overlap
    let hop_size = (window_size as f32 * (1.0 - overlap)) as usize;
    
    if samples.len() < window_size {
         return Err(anyhow::anyhow!("File too short (need at least {} samples)", window_size));
    }

    // Step 1: Compute STFT
    let stft_result = compute_stft(samples, window_size, hop_size, quiet)?;
    
    // Step 2: Compute spectral rolloff if requested
    let rolloff_frequencies = if compute_rolloff {
        Some(compute_spectral_rolloff(&stft_result, sample_rate, width))
    } else {
        None
    };
    
    // Step 3: Render to image
    let img = render_spectrogram(&stft_result, sample_rate, width, height, config, linear, quiet)?;
    
    Ok(SpectrogramResult {
        image: img,
        rolloff_frequencies,
        stft: stft_result,
    })
}

pub struct StftResult {
    // 2D array: time_frames x frequency_bins
    pub magnitudes: Vec<Vec<f32>>,
    pub num_time_frames: usize,
    pub num_freq_bins: usize,
}

fn compute_stft(samples: &[f32], window_size: usize, hop_size: usize, quiet: bool) -> Result<StftResult> {
    let num_time_frames = (samples.len() - window_size) / hop_size + 1;
    let num_freq_bins = window_size / 2;
    
    // Prepare window function (Hann) - pre-computed once
    let window: Vec<f32> = (0..window_size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (window_size as f32 - 1.0)).cos()))
        .collect();

    // Pre-compute FFT plan once and share across threads
    let mut planner = FftPlanner::new();
    let fft = Arc::new(planner.plan_fft_forward(window_size));

    // Setup progress bar
    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(num_time_frames as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} {msg} [{bar:40.cyan/blue}] {pos}/{len} frames ({percent}%)")
                .unwrap()
                .progress_chars("━━╸")
        );
        pb.set_message("STFT");
        pb
    };

    // Process frames in parallel with shared FFT plan
    let magnitudes: Vec<Vec<f32>> = (0..num_time_frames)
        .into_par_iter()
        .map(|frame_idx| {
            let start = frame_idx * hop_size;
            let end = start + window_size;
            
            // Use shared FFT plan
            let fft = Arc::clone(&fft);
            
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
            
            // Update progress (approximate in parallel)
            if frame_idx % 50 == 0 {
                pb.inc(50);
            }
            
            frame_mags
        })
        .collect();

    if !quiet {
        pb.finish_with_message("STFT ✓");
    }

    Ok(StftResult {
        magnitudes,
        num_time_frames,
        num_freq_bins,
    })
}

/// Compute spectral rolloff for each time frame
/// Rolloff is the frequency below which 85% of the total energy is contained
fn compute_spectral_rolloff(stft: &StftResult, sample_rate: u32, output_width: u32) -> Vec<f32> {
    let nyquist = sample_rate as f32 / 2.0;
    let rolloff_threshold = 0.85; // 85% threshold
    
    // Calculate rolloff per time frame
    let rolloff_per_frame: Vec<f32> = stft.magnitudes.par_iter()
        .map(|frame| {
            // Sum of squared magnitudes (energy)
            let total_energy: f32 = frame.iter().map(|m| m * m).sum();
            
            if total_energy < 1e-10 {
                return 0.0;
            }
            
            let threshold_energy = total_energy * rolloff_threshold;
            let mut cumulative_energy = 0.0;
            
            for (bin, &mag) in frame.iter().enumerate() {
                cumulative_energy += mag * mag;
                if cumulative_energy >= threshold_energy {
                    // Convert bin to frequency
                    let freq = (bin as f32 / stft.num_freq_bins as f32) * nyquist;
                    return freq;
                }
            }
            
            nyquist // All energy used, rolloff at max
        })
        .collect();
    
    // Resample to match output width (one value per pixel column)
    let num_frames = rolloff_per_frame.len();
    (0..output_width as usize)
        .map(|x| {
            let frame_pos = (x as f32 / output_width as f32) * num_frames as f32;
            let frame_idx = (frame_pos as usize).min(num_frames - 1);
            rolloff_per_frame[frame_idx]
        })
        .collect()
}

fn render_spectrogram(
    stft: &StftResult,
    sample_rate: u32,
    width: u32,
    height: u32,
    config: &Config,
    linear: bool,
    quiet: bool,
) -> Result<RgbImage> {
    let mut img = RgbImage::new(width, height);
    
    // Pre-compute gradient LUT once
    let gradient = create_gradient_map(&config.colors.stops, 1024);
    
    // Setup progress bar
    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(width as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} {msg} [{bar:40.cyan/blue}] {pos}/{len} cols ({percent}%)")
                .unwrap()
                .progress_chars("━━╸")
        );
        pb.set_message("Rendering");
        pb
    };

    // Constants for log scale
    let min_freq = 20.0; // 20 Hz
    let max_freq = sample_rate as f32 / 2.0;
    
    // Auto-Normalization (Dynamic Contrast)
    // Find global peak magnitude first using parallel reduction
    let global_max_mag = stft.magnitudes.par_iter()
        .map(|frame| {
            frame.iter().fold(0.0f32, |max, &val| max.max(val))
        })
        .reduce(|| 0.0f32, |a, b| a.max(b));
        
    // Convert max magnitude to dB for reference
    let max_mag_norm = global_max_mag / (stft.num_freq_bins as f32 / 2.0);
    let max_db = 20.0 * (max_mag_norm + 1e-9).log10();
    
    // Set dynamic range (100dB dynamic range below peak)
    let min_db = max_db - 100.0;
    let db_range = max_db - min_db;
    
    // Pre-compute values for inner loop
    let num_time_frames_f = stft.num_time_frames as f32;
    let num_freq_bins_f = stft.num_freq_bins as f32;
    let height_f = height as f32;
    let width_f = width as f32;
    let freq_ratio = max_freq / min_freq;
    let norm_factor = stft.num_freq_bins as f32 / 2.0;
    
    // Parallelize column processing
    let columns: Vec<(u32, Vec<Rgb<u8>>)> = (0..width)
        .into_par_iter()
        .map(|x| {
            // Map pixel x to time frame
            let time_pos = (x as f32 / width_f) * num_time_frames_f;
            
            // Pre-compute time interpolation indices
            let t0 = time_pos.floor() as usize;
            let t1 = (t0 + 1).min(stft.num_time_frames - 1);
            let t_fract = time_pos - t0 as f32;
            let t0 = t0.min(stft.num_time_frames - 1);

            let mut col_pixels = Vec::with_capacity(height as usize);

            for y in 0..height {
                // y=0 is top (high freq), y=height-1 is bottom (low freq)
                let y_inverted = height - 1 - y;
                let y_ratio = y_inverted as f32 / height_f;

                let bin_pos = if linear {
                    // Linear scale
                    y_ratio * num_freq_bins_f
                } else {
                    // Logarithmic scale
                    let freq = min_freq * freq_ratio.powf(y_ratio);
                    (freq / max_freq) * num_freq_bins_f
                };

                // Bilinear Interpolation
                let f0 = bin_pos.floor() as usize;
                let f1 = (f0 + 1).min(stft.num_freq_bins - 1);
                let f_fract = bin_pos - f0 as f32;
                let f0 = f0.min(stft.num_freq_bins - 1);
                
                // Get 4 samples for bilinear interpolation
                let m00 = stft.magnitudes[t0][f0];
                let m01 = stft.magnitudes[t0][f1];
                let m10 = stft.magnitudes[t1][f0];
                let m11 = stft.magnitudes[t1][f1];
                
                // Interpolate Time
                let m0 = m00 * (1.0 - t_fract) + m10 * t_fract;
                let m1 = m01 * (1.0 - t_fract) + m11 * t_fract;
                
                // Interpolate Freq
                let mag = m0 * (1.0 - f_fract) + m1 * f_fract;
            
                // Convert to dB
                let normalized_mag = mag / norm_factor;
                let db = 20.0 * (normalized_mag + 1e-9).log10();
            
                // Map dB to color using dynamic range
                let normalized_val = (db - min_db) / db_range;
                let clamped = normalized_val.max(0.0).min(1.0);
            
                let color_idx = (clamped * 1023.0) as usize;
                col_pixels.push(gradient[color_idx]);
            }
             
            (x, col_pixels)
        })
        .collect();
    
    // Assemble image
    for (x, col_pixels) in columns {
        for (y, pixel) in col_pixels.into_iter().enumerate() {
            img.put_pixel(x, y as u32, pixel);
        }
        pb.inc(1);
    }

    if !quiet {
        pb.finish_with_message("Render ✓");
    }

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

#[inline(always)]
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
