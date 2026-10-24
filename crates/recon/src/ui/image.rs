/// How the bytes of a captured image are arranged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelLayout {
    pub width: usize,
    pub height: usize,
    /// The length of one row in bytes. Rows may be padded past their last pixel, and the padding
    /// is not part of the image.
    pub bytes_per_row: usize,
    pub bytes_per_pixel: usize,
}

/// Reports whether every pixel of an image holds the same bytes, comparing only the first `width`
/// pixels of each row.
pub fn is_uniform(pixels: &[u8], layout: PixelLayout) -> bool {
    let PixelLayout { width, height, bytes_per_row, bytes_per_pixel } = layout;
    let mut image = (0..height).flat_map(|row| {
        let start = row * bytes_per_row;
        let end = start.saturating_add(width * bytes_per_pixel).min(pixels.len());
        pixels.get(start..end).unwrap_or_default().chunks_exact(bytes_per_pixel)
    });
    let Some(first) = image.next() else {
        return true;
    };
    image.all(|pixel| pixel == first)
}
