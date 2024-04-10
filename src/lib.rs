use std::io::Cursor;

use image::{
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
    guess_format,
    imageops::{resize, FilterType},
    io::{Limits, Reader},
    AnimationDecoder, Frame, ImageDecoder, ImageFormat, ImageResult, Pixel,
};

pub mod security;

pub fn resize_img(
    buf: &[u8],
    width: u32,
    height: u32,
    frames: usize,
) -> ImageResult<(Vec<u8>, ImageFormat)> {
    let cursor = Cursor::new(buf);
    let out: Vec<u8> = Vec::new();
    let mut write_buf = Cursor::new(out);
    let fmt = guess_format(buf)?;

    if fmt == ImageFormat::Gif {
        let mut decoder = GifDecoder::new(cursor)?;
        let mut limits = Limits::default();
        limits.free(512 * 1024);
        decoder.set_limits(limits)?;

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
        let resized = resize(&img, width, height, FilterType::Nearest);
        resized.write_to(&mut write_buf, ImageFormat::Png)?;
    }

    Ok((write_buf.into_inner(), fmt))
}

fn sx(x: i32, mid: i32) -> i32 {
    if x < mid {
        -1
    } else if x > mid {
        1
    } else {
        0
    }
}

fn sy(y: i32, mid: i32) -> i32 {
    if y < mid {
        1
    } else if y > mid {
        -1
    } else {
        0
    }
}

fn to_cartesian(x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
    let (w_mid, h_mid) = (width / 2, height / 2);
    (
        sx(x, w_mid) * (x - w_mid).abs(),
        sy(y, h_mid) * (y - h_mid).abs(),
    )
}

pub fn circlize_img(buf: &[u8], dim: u32) -> ImageResult<Vec<u8>> {
    let out: Vec<u8> = Vec::new();
    let mut write_buf = Cursor::new(out);

    let mut img = resize(
        &image::load_from_memory_with_format(buf, ImageFormat::Png)?.into_rgba8(),
        dim,
        dim,
        FilterType::Nearest,
    );
    let (w, h) = (dim as i32, dim as i32);
    let radius = dim / 2;
    for (x, y, px) in img.enumerate_pixels_mut().into_iter() {
        let (x_cart, y_cart) = to_cartesian(x as i32, y as i32, w, h);

        if ((x_cart.pow(2) + y_cart.pow(2)) as f32).sqrt() > (radius as f32) {
            px.apply(|_| 0);
        }
    }

    img.write_to(&mut write_buf, ImageFormat::Png)?;
    Ok(write_buf.into_inner())
}
