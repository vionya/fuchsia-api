use image::{
    imageops::{resize, FilterType},
    ImageBuffer, ImageFormat, ImageResult, Pixel, Rgba,
};

pub(crate) mod editor;
pub mod security;

#[inline]
fn scale_dims((width, height): (u32, u32), new_largest_dim: u32) -> (u32, u32) {
    let (width, height) = (width as f32, height as f32);
    let ratio = (new_largest_dim as f32) / width.max(height);
    (
        (ratio * width).floor() as u32,
        (ratio * height).floor() as u32,
    )
}

pub fn resize_img(
    buf: &[u8],
    mut width: u32,
    mut height: u32,
    frames: usize,
    keep_aspect: bool,
) -> ImageResult<(Vec<u8>, ImageFormat, (u32, u32))> {
    let mut editor = editor::Editor::new(None, frames);
    editor.set_buffer_processor(|img| {
        if keep_aspect {
            (width, height) = scale_dims(img.dimensions(), width.max(height))
        }
        resize(img, width, height, FilterType::Nearest)
    });
    editor.process(buf)
}

#[inline]
fn to_cartesian(x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
    let (w_mid, h_mid) = (width / 2, height / 2);
    (
        // calculate sign for x coord
        (x - w_mid).signum() * (x - w_mid).abs(), // multiply sign by distance from origin
        // calculate sign for y coord
        (h_mid - y).signum() * (y - h_mid).abs(), // multiply sign by distance from origin
    )
}

#[inline]
fn circlize_frame(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, dim: u32) {
    let (w, h) = (dim as i32, dim as i32);
    let radius = dim / 2;
    for (x, y, px) in img.enumerate_pixels_mut().into_iter() {
        // convert the coordinates to cartesian
        let (x_cart, y_cart) = to_cartesian(x as i32, y as i32, w, h);

        // determine if the current pixel lies outside of the circle
        if ((x_cart.pow(2) + y_cart.pow(2)) as f32).sqrt() > (radius as f32) {
            // if it does, make it transparent
            px.apply(|_| 0);
        }
    }
}

pub fn circlize_img(
    buf: &[u8],
    dim: u32,
    frames: usize,
) -> ImageResult<(Vec<u8>, ImageFormat, (u32, u32))> {
    let mut editor = editor::Editor::new(None, frames);
    editor.set_buffer_processor(|img| {
        let mut resized = resize(img, dim, dim, FilterType::Nearest);
        circlize_frame(&mut resized, dim);
        resized
    });
    editor.process(buf)
}
