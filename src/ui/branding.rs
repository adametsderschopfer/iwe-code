//! Decode the embedded window icon without filesystem access.

pub(crate) fn window_icon() -> Option<iced::window::Icon> {
    let (pixels, width, height) = decode_rgba(include_bytes!("../../assets/window-icon.png"))?;
    iced::window::icon::from_rgba(pixels, width, height).ok()
}

pub(super) fn decode_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    pixels.truncate(info.buffer_size());
    Some((pixels, info.width, info.height))
}

#[cfg(test)]
mod tests;
