use std::io::Cursor;

use image::{
    codecs::gif::{GifDecoder, GifEncoder, Repeat},
    guess_format,
    imageops::{resize, FilterType},
    io::{Limits, Reader},
    AnimationDecoder, Frame, ImageBuffer, ImageDecoder, ImageFormat, ImageResult, Pixel, Rgba,
};

pub mod security;

type RgbaImg = ImageBuffer<Rgba<u8>, Vec<u8>>;

struct Editor<'a, F>
where
    F: FnMut(&mut RgbaImg) -> RgbaImg,
{
    data: &'a [u8],
    cursor: Cursor<&'a [u8]>,
    write_buf: Cursor<Vec<u8>>,

    buffer_processor: Option<F>,
    frame_limit: usize,
}

impl<'a, F> Editor<'a, F>
where
    F: FnMut(&mut RgbaImg) -> RgbaImg,
{
    fn new(data: &'a [u8], frame_limit: usize) -> Self {
        let output: Vec<u8> = Vec::new();
        Self {
            data,
            cursor: Cursor::new(data),
            write_buf: Cursor::new(output),
            buffer_processor: None,
            frame_limit,
        }
    }

    fn set_buffer_processor(&mut self, processor: F) {
        self.buffer_processor = Some(processor);
    }

    fn process(mut self) -> ImageResult<(Vec<u8>, ImageFormat, (u32, u32))> {
        let mut processor = self
            .buffer_processor
            .expect("Called process without setting a processor");
        let fmt = guess_format(self.data)?;
        let mut new_dimensions: Option<(u32, u32)> = None;

        if fmt == ImageFormat::Gif {
            let mut decoder = GifDecoder::new(self.cursor)?;
            let mut limits = Limits::default();
            limits.free(512 * 1024);
            decoder.set_limits(limits)?;

            let frames: Vec<Frame> = decoder
                .into_frames()
                .take(self.frame_limit)
                .take_while(Result::is_ok)
                .map(|ele| {
                    // no need to worry about panic since we're only
                    // mapping over Ok items
                    let mut frame = ele.unwrap();
                    let buffer = frame.buffer_mut();
                    let processed_buffer = processor(buffer);
                    if new_dimensions.is_none() {
                        new_dimensions = Some(processed_buffer.dimensions());
                    }

                    Frame::from_parts(processed_buffer, frame.left(), frame.top(), frame.delay())
                })
                .collect();

            let mut encoder = GifEncoder::new(&mut self.write_buf);
            encoder.set_repeat(Repeat::Infinite)?;
            encoder.encode_frames(frames.into_iter())?;
        } else {
            let reader = Reader::new(self.cursor).with_guessed_format()?;
            let img = reader.decode()?;
            let processed_buffer = processor(&mut img.to_rgba8());
            new_dimensions = Some(processed_buffer.dimensions());
            processed_buffer.write_to(&mut self.write_buf, ImageFormat::Png)?;
        }

        Ok((
            self.write_buf.into_inner(),
            fmt,
            new_dimensions.unwrap_or((0, 0)),
        ))
    }
}

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
    let mut editor = Editor::new(buf, frames);
    editor.set_buffer_processor(|img| {
        if keep_aspect {
            (width, height) = scale_dims(img.dimensions(), width.max(height))
        }
        resize(img, width, height, FilterType::Nearest)
    });
    editor.process()
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
    let mut editor = Editor::new(buf, frames);
    editor.set_buffer_processor(|img| {
        let mut resized = resize(img, dim, dim, FilterType::Nearest);
        circlize_frame(&mut resized, dim);
        resized
    });
    editor.process()
}
