use proptest::prelude::*;
use recon::ui::image::is_uniform;

struct Image {
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    bytes_per_row: usize,
    bytes_per_pixel: usize,
}

impl Image {
    fn new(width: usize, height: usize, padding: usize, pixel: &[u8], filler: u8) -> Self {
        let bytes_per_pixel = pixel.len();
        let bytes_per_row = width * bytes_per_pixel + padding;
        let mut pixels = vec![filler; bytes_per_row * height];
        for row in 0..height {
            for column in 0..width {
                let start = row * bytes_per_row + column * bytes_per_pixel;
                pixels[start..start + bytes_per_pixel].copy_from_slice(pixel);
            }
        }
        Self { pixels, width, height, bytes_per_row, bytes_per_pixel }
    }

    fn is_uniform(&self) -> bool {
        is_uniform(&self.pixels, self.width, self.height, self.bytes_per_row, self.bytes_per_pixel)
    }

    fn change_byte(&mut self, pixel_index: usize, byte_in_pixel: usize) {
        let row = pixel_index / self.width;
        let column = pixel_index % self.width;
        let offset = row * self.bytes_per_row + column * self.bytes_per_pixel + byte_in_pixel;
        self.pixels[offset] = self.pixels[offset].wrapping_add(1);
    }
}

#[test]
fn an_image_whose_pixels_all_match_is_uniform() {
    let image = Image::new(4, 3, 0, &[0, 0, 0, 255], 0);
    assert!(image.is_uniform());
    assert!(is_uniform(&[1, 2, 3, 4], 1, 1, 4, 4), "one pixel always matches itself");
}

#[test]
fn row_padding_is_not_part_of_the_image() {
    let image = Image::new(4, 3, 12, &[17, 34, 51, 255], 0xAB);
    assert!(image.is_uniform());
}

#[test]
fn a_single_differing_pixel_is_not_uniform() {
    let mut image = Image::new(4, 3, 12, &[0, 0, 0, 0], 0);
    image.change_byte(7, 2);
    assert!(!image.is_uniform());

    let mut last = Image::new(4, 3, 0, &[9, 9, 9, 9], 0);
    last.change_byte(11, 3);
    assert!(!last.is_uniform());
}

#[test]
fn an_image_without_pixels_is_uniform() {
    assert!(is_uniform(&[], 0, 0, 0, 4));
    assert!(is_uniform(&[1, 2, 3, 4], 0, 5, 4, 4));
    assert!(is_uniform(&[9; 16], 5, 0, 4, 4));
}

proptest! {
    #[test]
    fn a_buffer_of_equal_pixels_is_uniform_at_any_size_and_stride(
        width in 1usize..17,
        height in 1usize..17,
        padding in 0usize..9,
        pixel in prop::collection::vec(any::<u8>(), 1..5),
        filler in any::<u8>(),
    ) {
        let image = Image::new(width, height, padding, &pixel, filler);
        prop_assert!(image.is_uniform());
    }

    #[test]
    fn changing_one_byte_of_one_pixel_breaks_uniformity(
        width in 1usize..17,
        height in 1usize..17,
        padding in 0usize..9,
        pixel in prop::collection::vec(any::<u8>(), 1..5),
        filler in any::<u8>(),
        choice in any::<prop::sample::Index>(),
        byte_choice in any::<prop::sample::Index>(),
    ) {
        prop_assume!(width * height >= 2);
        let mut image = Image::new(width, height, padding, &pixel, filler);
        let pixel_index = choice.index(width * height);
        image.change_byte(pixel_index, byte_choice.index(pixel.len()));
        prop_assert!(!image.is_uniform());
    }

    #[test]
    fn padding_bytes_never_change_the_answer(
        width in 1usize..17,
        height in 1usize..17,
        padding in 1usize..9,
        pixel in prop::collection::vec(any::<u8>(), 1..5),
        first_filler in any::<u8>(),
        second_filler in any::<u8>(),
        differing in prop::option::of(any::<prop::sample::Index>()),
    ) {
        let mut first = Image::new(width, height, padding, &pixel, first_filler);
        let mut second = Image::new(width, height, padding, &pixel, second_filler);
        if let Some(choice) = differing {
            let pixel_index = choice.index(width * height);
            first.change_byte(pixel_index, 0);
            second.change_byte(pixel_index, 0);
        }
        prop_assert_eq!(first.is_uniform(), second.is_uniform());
    }
}
