use wasm_minimal_protocol::*;
use image::GenericImageView;
use image::imageops::FilterType;
use color_quant::NeuQuant;
use std::str;
use std::collections::HashMap;
use serde::Deserialize;

initiate_protocol!();

#[derive(Deserialize)]
struct Config {
    width: Option<u32>,
    height: Option<u32>,
    scale: Option<f64>,
    colors: Option<i32>,
}

#[wasm_func]
pub fn rgba_to_grid(image_bytes: &[u8], config_bytes: &[u8]) -> Vec<u8> {
    let config: Config = match serde_json::from_slice(config_bytes) {
        Ok(c) => c,
        Err(_) => Config { width: Some(32), height: None, scale: None, colors: None },
    };

    let img = match image::load_from_memory(image_bytes) {
        Ok(i) => i,
        Err(_) => return b"{\"error\": \"Failed to load image data\"}".to_vec(),
    };

    let (orig_w, orig_h) = img.dimensions();
    if orig_w == 0 || orig_h == 0 { return b"{\"error\": \"Image 0 dim\"}".to_vec(); }

    let (target_width, target_height) = if let (Some(w), Some(h)) = (config.width, config.height) {
        (w, h)
    } else if let Some(w) = config.width {
        let scale = w as f64 / orig_w as f64;
        (w, (orig_h as f64 * scale) as u32)
    } else if let Some(h) = config.height {
        let scale = h as f64 / orig_h as f64;
        ((orig_w as f64 * scale) as u32, h)
    } else if let Some(s) = config.scale {
        let w = (orig_w as f64 * s) as u32;
        let h = (orig_h as f64 * s) as u32;
        (w, h)
    } else {
        let w = 32;
        let scale = w as f64 / orig_w as f64;
        (w, (orig_h as f64 * scale) as u32)
    };

    let target_width = target_width.max(1);
    let target_height = target_height.max(1);
    
    let resized = img.resize_exact(target_width, target_height, FilterType::Lanczos3);
    let pixels = resized.to_rgba8();
    let raw_pixels = pixels.as_raw();

    let max_colors = config.colors.unwrap_or(64).clamp(2, 256);
    let nq = NeuQuant::new(10, max_colors as usize, raw_pixels);
    
    let available_chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+=[]{}|;':,/<>"
        .chars()
        .collect();
    
    let mut palette_map = serde_json::Map::new();
    palette_map.insert(".".to_string(), serde_json::Value::Null);

    let color_map = nq.color_map_rgba(); 
    let mut index_to_char: HashMap<usize, char> = HashMap::new();
    let mut next_char_idx = 0; 

    let mut grid_str = String::new();

    for y in 0..target_height {
        for x in 0..target_width {
            let pixel = resized.get_pixel(x, y);
            let rgba = pixel.0;

            if rgba[3] < 128 {
                grid_str.push('.');
                continue;
            }

            let idx = nq.index_of(&rgba);
            
            let char_code = *index_to_char.entry(idx).or_insert_with(|| {
                if next_char_idx >= available_chars.len() {
                    return '?';
                }
                let c = available_chars[next_char_idx];
                next_char_idx += 1;
                
                let r = color_map[idx * 4];
                let g = color_map[idx * 4 + 1];
                let b = color_map[idx * 4 + 2];
                
                let hex_color = format!("#{}", hex::encode(&[r, g, b]));
                palette_map.insert(c.to_string(), serde_json::Value::String(hex_color));
                
                c
            });

            grid_str.push(char_code);
        }
        grid_str.push('\n');
    }

    let mut final_json_obj = serde_json::Map::new();
    let trimmed_grid = grid_str.trim_end().to_string();
    final_json_obj.insert("art".to_string(), serde_json::Value::String(trimmed_grid));
    final_json_obj.insert("palette".to_string(), serde_json::Value::Object(palette_map));

    serde_json::to_vec(&final_json_obj).unwrap_or(b"{\"error\": \"JSON Serialization failed\"}".to_vec())
}