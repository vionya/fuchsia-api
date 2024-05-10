use std::io::Cursor;

use image::{
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
    guess_format,
    imageops::{resize, FilterType},
    io::{Limits, Reader},
    AnimationDecoder, Frame, GenericImageView, ImageDecoder, ImageFormat, ImageResult, Pixel,
};

pub mod security;

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
    width: u32,
    height: u32,
    frames: usize,
    keep_aspect: bool,
) -> ImageResult<(Vec<u8>, ImageFormat, (u32, u32))> {
    let cursor = Cursor::new(buf);
    let out: Vec<u8> = Vec::new();
    let mut write_buf = Cursor::new(out);
    let fmt = guess_format(buf)?;
    let (mut width, mut height) = (width, height);

    if fmt == ImageFormat::Gif {
        let mut decoder = GifDecoder::new(cursor)?;
        let mut limits = Limits::default();
        limits.free(512 * 1024);
        decoder.set_limits(limits)?;
        if keep_aspect {
            (width, height) = scale_dims(decoder.dimensions(), width.max(height))
        }

        let frames: Vec<Frame> = decoder
            .into_frames()
            .take(frames)
            .take_while(Result::is_ok)
            .map(|ele| {
                // no need to worry about panic since we're only
                // mapping over Ok items
                let mut frame = ele.unwrap();
                let buffer = frame.buffer_mut();

                Frame::from_parts(
                    resize(buffer, width, height, FilterType::Nearest),
                    frame.left(),
                    frame.top(),
                    frame.delay(),
                )
            })
            .collect();

        let mut encoder = GifEncoder::new(&mut write_buf);
        encoder.set_repeat(Repeat::Infinite)?;
        encoder.encode_frames(frames.into_iter())?;
    } else {
        let reader = Reader::new(cursor).with_guessed_format()?;
        let img = reader.decode()?;
        if keep_aspect {
            (width, height) = scale_dims(img.dimensions(), width.max(height))
        }
        let resized = resize(&img, width, height, FilterType::Nearest);
        resized.write_to(&mut write_buf, ImageFormat::Png)?;
    }

    Ok((write_buf.into_inner(), fmt, (width, height)))
}

fn to_cartesian(x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
    let (w_mid, h_mid) = (width / 2, height / 2);
    (
        // calculate sign for x coord
        (x - w_mid).signum() * (x - w_mid).abs(), // multiply sign by distance from origin
        // calculate sign for y coord
        (h_mid - y).signum() * (y - h_mid).abs(), // multiply sign by distance from origin
    )
}

pub fn circlize_img(buf: &[u8], dim: u32) -> ImageResult<Vec<u8>> {
    let out: Vec<u8> = Vec::new();
    let mut write_buf = Cursor::new(out);

    // resize image to the correct dimensions
    let mut img = resize(
        &image::load_from_memory_with_format(buf, ImageFormat::Png)?.into_rgba8(),
        dim,
        dim,
        FilterType::Nearest,
    );
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

    img.write_to(&mut write_buf, ImageFormat::Png)?;
    Ok(write_buf.into_inner())
}
