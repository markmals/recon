/// Reports whether every pixel of an image holds the same bytes.
///
/// Rows of a captured image are padded to `bytes_per_row`, and the padding is not part of the
/// image, so only the first `width` pixels of each row are compared.
pub fn is_uniform(
    pixels: &[u8],
    width: usize,
    height: usize,
    bytes_per_row: usize,
    bytes_per_pixel: usize,
) -> bool {
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
