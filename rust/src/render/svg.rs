use core::fmt::{Display, Formatter};

use crate::qr_code::QrCode;

impl QrCode {
    /// Wrap into an [`QrAsSvg`] struct, that can be written anywhere in the form of an optimized SVG.
    pub fn to_svg(&self, padding: u8) -> QrAsSvg<'_> {
        QrAsSvg { qr: self, padding }
    }
}

/// Implements the [`Display`](std::fmt::Display) trait.
///
/// It returns a string of SVG code for an image depicting the given QR Code.
///
/// Padding (the white border) is used as some QR Code readers don't work without it.
///
/// The string always uses Unix newlines `\n`, regardless of the platform.
pub struct QrAsSvg<'a> {
    pub qr: &'a QrCode,
    pub padding: u8,
}

impl Display for QrAsSvg<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let padding = self.padding as i32;
        let dimension = self.qr.size() + padding * 2;

        write!(
            f,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {dimension} {dimension}\">"
        )?;
        write!(f, "<rect width=\"100%\" height=\"100%\" fill=\"#fff\"/>")?;
        f.write_str("<path d=\"")?;

        let mut new_row;
        let mut last_cursor_position = 0;

        for y in 0..self.qr.size() {
            new_row = true;

            for x in 0..self.qr.size() {
                if self.qr.get_module(x, y) {
                    let dx = x + padding;
                    let dy = y + padding;

                    // On new rows we move the cursor to the next black box with "M[x] [y]"
                    if new_row {
                        new_row = false;
                        write!(f, "M{dx} {dy}h1v1{horizontal}z", horizontal = Horizontal(dx))?;

                    // On a box that's within the same row we move the cursor by
                    // [x] - last_cursor_position with "m[delta] 0" because that's shorter.
                    // Example:
                    //   M1 1, M2 2 .. M9 9, M10 10, M11 11 .. M99 99, M100 100, M101 101
                    //   vs
                    //   m1 0, m2 0 .. m9 0, m10 0, m11 0 .. m99 0, m100 0, m101 0
                    } else {
                        write!(
                            f,
                            "m{x} 0h1v1{horizontal}z",
                            x = dx - last_cursor_position,
                            horizontal = Horizontal(dx),
                        )?;
                    }
                    last_cursor_position = dx;
                }
            }
        }

        f.write_str("\"/></svg>\n")?;
        Ok(())
    }
}

struct Horizontal(i32);

impl Display for Horizontal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            // "H[x]" is shorter when x < 9
            //   H1 H2 .. H9 H10 H11 .. H99 H100 H101
            0..=9 => write!(f, "H{dx}", dx = self.0),
            // "h-1" is at least the same length or shorter when x > 9
            //   h-1 h-1 .. h-1 h-1 h-1 .. h-1 h-1 h-1
            _ => f.write_str("h-1"),
        }
    }
}
