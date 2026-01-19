use anyhow::Result;
use image::{RgbImage, Rgb};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut, draw_filled_rect_mut};
use imageproc::rect::Rect;
use rusttype::{Font, Scale};
use std::process::Command;
use std::path::PathBuf;
use crate::config::{Config, ColorStop};

/// Layout constants
const LEGEND_WIDTH: u32 = 60;       // Width of color bar on right
const LEGEND_PADDING: u32 = 10;      // Padding around legend
const LABEL_MARGIN: i32 = 50;        // Margin to avoid label overlap

/// Rendering options for the final image
pub struct RenderOptions {
    pub linear: bool,
    pub show_rolloff: bool,
    pub rolloff_frequencies: Option<Vec<f32>>, // Hz per time frame
}

/// Prepare the final image with overlays and optional color bar
pub fn prepare_final_image(
    spectrogram: RgbImage, 
    sample_rate: u32, 
    duration_secs: f64, 
    config: &Config, 
    options: RenderOptions,
) -> Result<RgbImage> {
    let font = load_font(config)?;
    let font = match font {
        Some(f) => f,
        None => return Ok(spectrogram),
    };

    let spec_width = spectrogram.width();
    let spec_height = spectrogram.height();
    
    // Create wider image to accommodate color bar on the right
    let total_width = spec_width + LEGEND_WIDTH + LEGEND_PADDING;
    let mut img = RgbImage::from_pixel(total_width, spec_height, Rgb([0, 0, 0]));
    
    // Copy spectrogram to left portion
    for y in 0..spec_height {
        for x in 0..spec_width {
            img.put_pixel(x, y, *spectrogram.get_pixel(x, y));
        }
    }

    let font_size = 20.0;
    let small_font_size = 14.0;
    let scale = Scale { x: font_size, y: font_size };
    let small_scale = Scale { x: small_font_size, y: small_font_size };
    let text_color = Rgb([255, 255, 255]);
    let outline_color = Rgb([0, 0, 0]);
    let line_color = Rgb([200, 200, 200]); 
    let rolloff_color = Rgb([255, 200, 50]); // Yellow/orange for rolloff line
    
    // Helper to draw outlined text
    let draw_outlined_text = |img: &mut RgbImage, text: &str, x: i32, y: i32, s: Scale| {
        for ox in -1..=1 {
            for oy in -1..=1 {
                if ox != 0 || oy != 0 {
                    draw_text_mut(img, outline_color, x + ox, y + oy, s, &font, text);
                }
            }
        }
        draw_text_mut(img, text_color, x, y, s, &font, text);
    };

    let nyquist = sample_rate as f32 / 2.0;

    // Draw frequency axis labels
    draw_frequency_axis(
        &mut img, 
        sample_rate, 
        options.linear, 
        spec_height, 
        line_color, 
        &|img, text, x, y| draw_outlined_text(img, text, x, y, scale)
    );

    // Draw time axis labels
    draw_time_axis(
        &mut img, 
        duration_secs, 
        spec_width, 
        spec_height, 
        line_color, 
        &|img, text, x, y| draw_outlined_text(img, text, x, y, scale)
    );

    // Draw axis title labels (small, subtle)
    // "Hz" near top-left corner
    draw_outlined_text(&mut img, "Hz", 5, 5, small_scale);
    
    // "Time" near bottom-right of spectrogram area
    let time_label_x = (spec_width as i32) - 40;
    let time_label_y = (spec_height as i32) - 18;
    draw_outlined_text(&mut img, "Time", time_label_x, time_label_y, small_scale);

    // Draw scale type indicator (top-right corner of spectrogram)
    let scale_label = if options.linear { "LINEAR" } else { "LOG" };
    let scale_x = (spec_width as i32) - 55;
    draw_outlined_text(&mut img, scale_label, scale_x, 5, small_scale);

    // Draw spectral rolloff line if enabled
    if options.show_rolloff {
        if let Some(ref rolloff_freqs) = options.rolloff_frequencies {
            draw_rolloff_line(
                &mut img, 
                rolloff_freqs, 
                spec_width, 
                spec_height, 
                nyquist, 
                options.linear, 
                rolloff_color
            );
        }
    }

    // Draw color bar / legend on the right side
    draw_color_bar(
        &mut img,
        &config.colors.stops,
        spec_width,
        spec_height,
        &|img, text, x, y| draw_outlined_text(img, text, x, y, small_scale)
    );

    Ok(img)
}

fn draw_frequency_axis<F>(
    img: &mut RgbImage,
    sample_rate: u32,
    linear: bool,
    height: u32,
    line_color: Rgb<u8>,
    draw_text: &F,
) where F: Fn(&mut RgbImage, &str, i32, i32) {
    let nyquist = sample_rate as f32 / 2.0;
    let height_i = height as i32;

    if linear {
        let step_khz = 5.0;
        let mut freq = 0.0;
        
        while freq <= nyquist / 1000.0 {
            let y_ratio = freq * 1000.0 / nyquist;
            let y_pos = (height as f32 * (1.0 - y_ratio)) as i32;
            
            // Skip if too close to bottom edge (overlap zone)
            if y_pos >= 0 && y_pos < height_i && y_pos < height_i - LABEL_MARGIN {
                draw_line_segment_mut(img, (0.0, y_pos as f32), (10.0, y_pos as f32), line_color);
                let label = format!("{}k", freq as i32);
                draw_text(img, &label, 15, y_pos - 10);
            }
            freq += step_khz;
        }
    } else {
        let freqs = [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
        let min_freq = 20.0;
        let max_freq = nyquist;

        for &freq in freqs.iter() {
            if freq > max_freq { break; }
            
            let y_ratio = (freq / min_freq).log10() / (max_freq / min_freq).log10();
            let y_pos = (height as f32 - 1.0 - (y_ratio * height as f32)) as i32;

            // Skip if too close to bottom edge (overlap zone)
            if y_pos >= 0 && y_pos < height_i && y_pos < height_i - LABEL_MARGIN {
                draw_line_segment_mut(img, (0.0, y_pos as f32), (10.0, y_pos as f32), line_color);
                
                let label = if freq >= 1000.0 {
                    format!("{}k", freq / 1000.0)
                } else {
                    format!("{}", freq as i32)
                };
                
                draw_text(img, &label, 15, y_pos - 10);
            }
        }
    }
}

fn draw_time_axis<F>(
    img: &mut RgbImage,
    duration_secs: f64,
    width: u32,
    height: u32,
    line_color: Rgb<u8>,
    draw_text: &F,
) where F: Fn(&mut RgbImage, &str, i32, i32) {
    let step_secs = if duration_secs < 60.0 { 10.0 } else { 30.0 };
    let width_i = width as i32;
    let height_f = height as f32;
    let mut t = 0.0;
    
    while t <= duration_secs {
        let x_ratio = t / duration_secs;
        let x_pos = (width as f32 * x_ratio as f32) as i32;
        
        if x_pos >= 0 && x_pos < width_i {
            draw_line_segment_mut(
                img, 
                (x_pos as f32, height_f), 
                (x_pos as f32, height_f - 10.0), 
                line_color
            );
            
            let minutes = (t / 60.0).floor() as i32;
            let seconds = (t % 60.0) as i32;
            let label = format!("{}:{:02}", minutes, seconds);
            
            // Offset first label to the right, others centered around tick
            let text_x = if t == 0.0 { x_pos + 5 } else { x_pos - 15 };
            draw_text(img, &label, text_x, height as i32 - 28);
        }
        t += step_secs;
    }
}

fn draw_rolloff_line(
    img: &mut RgbImage,
    rolloff_freqs: &[f32],
    width: u32,
    height: u32,
    nyquist: f32,
    linear: bool,
    color: Rgb<u8>,
) {
    let min_freq = 20.0f32;
    let height_f = height as f32;
    
    let mut prev_point: Option<(f32, f32)> = None;
    
    for (i, &freq) in rolloff_freqs.iter().enumerate() {
        let x = (i as f32 / rolloff_freqs.len() as f32) * width as f32;
        
        // Convert frequency to Y position
        let y = if linear {
            let y_ratio = freq / nyquist;
            height_f * (1.0 - y_ratio)
        } else {
            if freq < min_freq {
                height_f - 1.0
            } else {
                let y_ratio = (freq / min_freq).log10() / (nyquist / min_freq).log10();
                height_f - 1.0 - (y_ratio * height_f)
            }
        };
        
        let y = y.max(0.0).min(height_f - 1.0);
        
        if let Some((px, py)) = prev_point {
            draw_line_segment_mut(img, (px, py), (x, y), color);
        }
        
        prev_point = Some((x, y));
    }
}

fn draw_color_bar<F>(
    img: &mut RgbImage,
    stops: &[ColorStop],
    spec_width: u32,
    height: u32,
    draw_text: &F,
) where F: Fn(&mut RgbImage, &str, i32, i32) {
    let bar_x = spec_width + LEGEND_PADDING;
    let bar_width = 15;
    let bar_margin = 20;
    let bar_height = height - 2 * bar_margin;
    
    // Create gradient for the bar
    let gradient = create_gradient_map(stops, bar_height as usize);
    
    // Draw the color bar (reversed: top = high dB, bottom = low dB)
    for y in 0..bar_height {
        let color = gradient[(bar_height - 1 - y) as usize];
        draw_filled_rect_mut(
            img, 
            Rect::at(bar_x as i32, (bar_margin + y) as i32).of_size(bar_width, 1), 
            color
        );
    }
    
    // Draw border around bar
    let border_color = Rgb([150, 150, 150]);
    draw_line_segment_mut(img, (bar_x as f32, bar_margin as f32), ((bar_x + bar_width) as f32, bar_margin as f32), border_color);
    draw_line_segment_mut(img, (bar_x as f32, (bar_margin + bar_height) as f32), ((bar_x + bar_width) as f32, (bar_margin + bar_height) as f32), border_color);
    draw_line_segment_mut(img, (bar_x as f32, bar_margin as f32), (bar_x as f32, (bar_margin + bar_height) as f32), border_color);
    draw_line_segment_mut(img, ((bar_x + bar_width) as f32, bar_margin as f32), ((bar_x + bar_width) as f32, (bar_margin + bar_height) as f32), border_color);
    
    // Draw dB labels
    let label_x = (bar_x + bar_width + 3) as i32;
    draw_text(img, "0dB", label_x, bar_margin as i32);
    draw_text(img, "-50", label_x, (bar_margin + bar_height / 2) as i32 - 5);
    draw_text(img, "-100", label_x, (bar_margin + bar_height) as i32 - 12);
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

fn load_font(config: &Config) -> Result<Option<Font<'static>>> {
    let font_path = config.font_path.clone()
        .or_else(get_system_font_path)
        .or_else(|| {
            [
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/System/Library/Fonts/Helvetica.ttc",
                "C:\\Windows\\Fonts\\arial.ttf"
            ].iter()
             .map(PathBuf::from)
             .find(|p| p.exists())
        });

    match font_path {
        Some(path) if path.exists() => {
            let font_data = std::fs::read(&path)?;
            Ok(Font::try_from_vec(font_data))
        }
        _ => Ok(None),
    }
}

fn get_system_font_path() -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .arg("--format=%{file}")
        .output()
        .ok()?;
        
    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}
