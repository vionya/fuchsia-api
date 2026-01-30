use std::io::{Cursor, Write};

use image::{
    codecs::
        gif::{GifDecoder, GifEncoder, Repeat}
    ,
    error::{EncodingError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
    guess_format,
    io::{Limits, Reader},
    AnimationDecoder, DynamicImage, Frame, ImageBuffer, ImageDecoder, ImageError, ImageFormat,
    ImageResult, Rgba, RgbaImage,
};
use log::info;
use webp::{AnimDecoder, AnimEncoder, AnimFrame, WebPConfig};

type RgbaImg = ImageBuffer<Rgba<u8>, Vec<u8>>;

pub struct Editor<'a, F>
where
    F: FnMut(&mut RgbaImg) -> RgbaImg,
{
    accepted_types: &'a [ImageFormat],
    write_buf: Cursor<Vec<u8>>,
    buffer_processor: Option<F>,
    frame_limit: usize,
}

impl<'a, F> Editor<'a, F>
where
    F: FnMut(&mut RgbaImg) -> RgbaImg,
{
    /// Create a new [`Editor`] with a given frame processing limit (for GIF inputs)
    pub fn new(accepted_types: Option<&'a [ImageFormat]>, frame_limit: usize) -> Self {
        let output: Vec<u8> = Vec::new();
        Self {
            accepted_types: accepted_types.unwrap_or(&[
                ImageFormat::Png,
                ImageFormat::Jpeg,
                ImageFormat::WebP,
                ImageFormat::Gif,
            ]),
            write_buf: Cursor::new(output),
            buffer_processor: None,
            frame_limit,
        }
    }

    /// Register a callback function to process an individual image or each frame of a GIF input
    pub fn set_buffer_processor(&mut self, processor: F) {
        self.buffer_processor = Some(processor);
    }

    /// Process image data with this editor
    ///
    /// Panics if a processor callback has not been registered to this [`Editor`]
    pub fn process(mut self, data: &[u8]) -> ImageResult<(Vec<u8>, ImageFormat, (u32, u32))> {
        let mut processor = self
            .buffer_processor
            .expect("Called process without setting a processor");
        let cursor = Cursor::new(data);
        let fmt = guess_format(data)?;
        // TODO: this sucks lmao
        if !self.accepted_types.contains(&fmt) {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Exact(fmt),
                    UnsupportedErrorKind::Format(ImageFormatHint::Exact(fmt)),
                ),
            ));
        }
        let mut new_dimensions: Option<(u32, u32)> = None;

        if fmt == ImageFormat::WebP {
            let err =
                ImageError::Encoding(EncodingError::from_format_hint(ImageFormatHint::Exact(fmt)));

            let decoder = AnimDecoder::new(cursor.into_inner());
            let Ok(decoded) = decoder.decode() else {
                return Err(err);
            };
            let Some(original_frames) = decoded.get_frames(0..(decoded.len())) else {
                return Err(err);
            };

            let frames: Vec<DynamicImage> = original_frames
                .iter()
                .take(self.frame_limit)
                .map(|frame| {
                    // no need to worry about panic since we're only
                    // mapping over Ok items
                    // let mut frame = ele.unwrap();
                    let mut buffer = RgbaImage::from_raw(
                        frame.width(),
                        frame.height(),
                        frame.get_image().to_vec(),
                    )
                    .expect("surely this wont happen");
                    let processed_buffer = processor(&mut buffer);
                    if new_dimensions.is_none() {
                        new_dimensions = Some(processed_buffer.dimensions());
                    }
                    DynamicImage::from(processed_buffer)
                })
                .collect();

            let err =
                ImageError::Encoding(EncodingError::from_format_hint(ImageFormatHint::Exact(fmt)));
            let (w, h) = new_dimensions.unwrap_or_else(|| (256, 256));
            let Ok(config) = WebPConfig::new() else {
                return Err(err);
            };
            let mut encoder = AnimEncoder::new(w, h, &config);
            info!("{} frames in frames vec", frames.len());
            frames.iter().enumerate().for_each(|(i, frame)| {
                let Ok(anim_frame) = AnimFrame::from_image(frame, i as i32) else {
                    return;
                };
                encoder.add_frame(anim_frame)
            });
            info!("all frames added");
            self.write_buf.write(&encoder.encode())?;
            info!("encoding should be done");
        } else if fmt == ImageFormat::Gif {
            let mut decoder = GifDecoder::new(cursor)?;
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
            let reader = Reader::new(cursor).with_guessed_format()?;
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
