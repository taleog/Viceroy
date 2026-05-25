use anyhow::{anyhow, Result};
use std::io::Cursor;

pub const APP_ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");
pub const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/icon-tray.png");

pub fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|err| anyhow!("failed to read PNG metadata: {err}"))?;

    let output_buffer_size = reader.output_buffer_size();
    let mut buffer = vec![0; output_buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|err| anyhow!("failed to decode PNG: {err}"))?;

    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buffer[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
            for chunk in rgb.chunks_exact(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let gray = &buffer[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
            for &value in gray {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            let gray_alpha = &buffer[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
            for chunk in gray_alpha.chunks_exact(2) {
                rgba.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            rgba
        }
        png::ColorType::Indexed => {
            return Err(anyhow!("indexed PNGs are not supported for logo assets"));
        }
    };

    Ok((info.width, info.height, rgba))
}
