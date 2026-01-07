use anyhow::Result;
use rustfft::{FftPlanner, num_complex::Complex};
use image::{RgbImage, Rgb};
use crate::config::ColorStop;
use crate::config::Config;

pub fn generate_spectrogram(
    samples: &[f32],
    _sample_rate: u32,
    width: u32,
    height: u32,
    config: &Config,
) -> Result<RgbImage> {
    let window_size = 2048; 
    
    if samples.len() < window_size {
         return Err(anyhow::anyhow!("File too short"));
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(window_size);

    // Prepare window function (Hann)
    let window: Vec<f32> = (0..window_size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (window_size as f32 - 1.0)).cos()))
        .collect();

    let mut img = RgbImage::new(width, height);
    
    let gradient = create_gradient_map(&config.colors.stops, 1024);

    let spectrum_len = window_size / 2;
    // Pre-allocate buffer for reuse if we were iterating differently, but here we do per-column.
    
    // We iterate over the image width (pixels)
    for x in 0..width {
        // Calculate the center sample index for this pixel column
        let center_sample = (x as f64 / width as f64 * samples.len() as f64) as usize;
        
        let start_idx = if center_sample < window_size / 2 { 0 } else { center_sample - window_size / 2 };
        let end_idx = start_idx + window_size;
        
        if end_idx > samples.len() {
             break; 
        }

        let mut buffer: Vec<Complex<f32>> = samples[start_idx..end_idx]
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| Complex { re: s * w, im: 0.0 })
            .collect();
        
        if buffer.len() < window_size {
             buffer.resize(window_size, Complex { re: 0.0, im: 0.0 });
        }

        fft.process(&mut buffer);

        // Process this column
        // We have `spectrum_len` bins (0 to Nyquist).
        // We need to map to `height` pixels.
        
        for y in 0..height {
            // y=0 is top (High Freq), y=height-1 is bottom (Low Freq).
            // Let's invert y to get low->high (0..height).
            let y_inverted = height - 1 - y;
            
            // Determine frequency range for this pixel
            // Each pixel covers a range of bins.
            let bin_start_f = (y_inverted as f32 / height as f32) * spectrum_len as f32;
            let bin_end_f = ((y_inverted + 1) as f32 / height as f32) * spectrum_len as f32;
            
            let bin_start = bin_start_f.floor() as usize;
            let bin_end = bin_end_f.ceil() as usize;
            
            // To avoid missing peaks when scaling down (height < spectrum_len), we take the MAX magnitude in the range.
            // If scaling up (height > spectrum_len), bin_start might equal bin_end or be close.
            
            let effective_end = bin_end.max(bin_start + 1).min(spectrum_len);
            let effective_start = bin_start.min(effective_end - 1);
            
            let mut max_mag: f32 = 0.0;
            
            for i in effective_start..effective_end {
                let c = buffer[i];
                let mag = c.norm(); // equivalent to sqrt(re^2 + im^2)
                if mag > max_mag {
                    max_mag = mag;
                }
            }

            // Normalize
            // Max theoretical magnitude for Hann window of size N on sine wave is N/2 * 0.5 = N/4?
            // Actually sum of window is N/2. 
            // Let's normalize by window_size/2 roughly.
            let normalized_mag = max_mag / (window_size as f32 / 2.0);
            
            // dB
            let db = 20.0 * (normalized_mag + 1e-9).log10();
            
            // Clamp and map
            let min_db = -100.0; // Noise floor
            let max_db = 0.0;    // Peak
            
            let normalized_val = (db - min_db) / (max_db - min_db);
            let clamped = normalized_val.max(0.0).min(1.0);
            
            let color_idx = (clamped * 1023.0) as usize;
            let pixel = gradient[color_idx];
            
            img.put_pixel(x, y, pixel);
        }
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
