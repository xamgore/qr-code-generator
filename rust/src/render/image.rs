use image::{ImageBuffer, ImageEncoder, ImageResult, Luma};

use crate::prelude::QrCode;

pub struct QrAsImageBuffer<'a> {
    pub qr: &'a QrCode,
    pub padding: u8,
    pub image_size: u32,
    pub buffer: ImageBuffer<Luma<u8>, Vec<u8>>,
}

impl QrAsImageBuffer<'_> {
    pub fn write_image<E: ImageEncoder>(&self, encoder: E) -> ImageResult<()> {
        encoder.write_image(
            self.buffer.as_raw(),
            self.image_size,
            self.image_size,
            image::ExtendedColorType::L8,
        )
    }
}

impl QrCode {
    /// Returns an image buffer depicting the given QR Code.
    ///
    /// Padding (the white border) is used as some QR Code readers don't work without it.
    ///
    /// # Example
    ///
    /// Generate a base64-encoded text string containing a PNG image of the QR Code.
    ///
    /// ```ignored
    /// use base64::{engine::general_purpose, Engine as _};
    ///
    /// let mut buf = Vec::new();
    /// let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    ///
    /// qr.to_image_buffer(4)
    ///     .write_image(encoder)
    ///     .map(|buf| general_purpose::STANDARD.encode(buf))
    ///     .ok();
    /// ```
    pub fn to_image_buffer(&self, padding: u8) -> QrAsImageBuffer<'_> {
        let padding = padding as u32;
        let size = self.size() as u32;

        let image_size = (size + padding * 2) * 8;
        let mut canvas = image::GrayImage::from_pixel(image_size, image_size, Luma([255]));

        let raw = canvas.as_mut();

        // The QR inside the white border
        for x_qr in 0..size {
            for y_qr in 0..size {
                if !self.get_module(x_qr as i32, y_qr as i32) {
                    continue;
                }

                // Multiply coordinates by width of pixels
                // And take into account the padding on top and left side
                let x_start = (x_qr + padding) * 8;
                let y_start = (y_qr + padding) * 8;

                // Draw an 8-pixels-wide square
                for y_img in y_start..y_start + 8 {
                    let start = (x_start + y_img * image_size) as usize;
                    raw[start..start + 8].copy_from_slice(&[0; 8]);
                }
            }
        }

        QrAsImageBuffer {
            qr: self,
            padding: padding as u8,
            buffer: canvas,
            image_size,
        }
    }
}
