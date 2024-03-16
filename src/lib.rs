use std::io::Cursor;

use image::{
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
    guess_format,
    imageops::{resize, FilterType},
    io::{Limits, Reader},
    AnimationDecoder, Frame, ImageDecoder, ImageFormat, ImageResult,
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
