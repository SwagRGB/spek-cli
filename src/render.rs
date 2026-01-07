use anyhow::Result;
use image::{RgbImage, Rgb};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut};
use rusttype::{Font, Scale};
use std::process::Command;
use std::path::PathBuf;
use crate::config::Config;

pub fn draw_labels(img: &mut RgbImage, sample_rate: u32, duration_secs: f64, config: &Config) -> Result<()> {
    // Determine font path priority:
    // 1. Config
    // 2. fc-match (System Default)
    // 3. Fallback hardcoded paths
    
    let font_path = if let Some(path) = &config.font_path {
        Some(path.clone())
    } else {
        get_system_font_path()
    };
    
    // Fallbacks
    let font_path = font_path.or_else(|| {
        [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/System/Library/Fonts/Helvetica.ttc", // MacOS
            "C:\\Windows\\Fonts\\arial.ttf" // Windows
        ].iter()
         .map(PathBuf::from)
         .find(|p| p.exists())
    });

    let font = if let Some(path) = font_path {
        if path.exists() {
             let font_data = std::fs::read(&path)?;
             Font::try_from_vec(font_data)
        } else {
             None
        }
    } else {
        None
    };
    
    // Check if we got a valid font
    let font = match font {
        Some(f) => f,
        None => {
            eprintln!("Warning: No suitable font found. Skipping labels.");
            return Ok(());
        }
    };

    let width = img.width();
    let height = img.height();
    let scale = Scale { x: 14.0, y: 14.0 };
    let text_color = Rgb([255, 255, 255]);
    let line_color = Rgb([200, 200, 200]);

    // Draw Frequency Labels (Y-axis)
    let nyquist = sample_rate as f32 / 2.0;
    let step_khz = 5.0; // 5kHz steps
    
    let mut freq = 0.0;
    while freq <= nyquist / 1000.0 {
        let y_ratio = freq * 1000.0 / nyquist; // 0 to 1
        let y_pixel = (height as f32 * (1.0 - y_ratio)) as i32;
        
        if y_pixel >= 0 && y_pixel < height as i32 {
            // Draw tick
            draw_line_segment_mut(img, (0.0, y_pixel as f32), (10.0, y_pixel as f32), line_color);
            
            // Draw text
            let label = format!("{}k", freq);
            let text_y = if y_pixel < 10 { 0 } else { y_pixel - 7 };
            draw_text_mut(img, text_color, 12, text_y, scale, &font, &label);
        }
        
        freq += step_khz;
    }

    // Draw Time Labels (X-axis)
    let step_secs = if duration_secs < 60.0 { 10.0 } else { 30.0 };
    
    let mut t = 0.0;
    while t <= duration_secs {
        let x_ratio = t / duration_secs;
        let x_pixel = (width as f32 * x_ratio as f32) as i32;
        
        if x_pixel >= 0 && x_pixel < width as i32 {
             // Draw tick
             draw_line_segment_mut(img, (x_pixel as f32, height as f32 - 10.0), (x_pixel as f32, height as f32), line_color);
             
             // Draw text
             let minutes = (t / 60.0).floor();
             let seconds = t % 60.0;
             let label = format!("{}:{:02}", minutes, seconds as i32);
             
             let text_x = if x_pixel > (width as i32 - 30) { x_pixel - 30 } else { x_pixel + 2 };
             draw_text_mut(img, text_color, text_x, height as i32 - 20, scale, &font, &label);
        }
        t += step_secs;
    }

    Ok(())
}

fn get_system_font_path() -> Option<PathBuf> {
    // Try to use `fc-match` to get the default font file path
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
