use std::convert::TryFrom;

use QrSegmentMode::*;

use crate::bit_buffer::BitBuffer;
use crate::version::Version;

/// The set of all legal characters in alphanumeric mode,
/// where each character value maps to the index in the string.
static ALPHANUMERIC_CHARSET: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// Describes how a segment's data bits are interpreted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QrSegmentMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    Eci,
}

/// A segment of character/binary/control data in a QR Code symbol.
///
/// Instances of this struct are immutable.
///
/// The mid-level way to create a segment is to take the payload data
/// and call a static factory function such as `QrSegment::make_numeric()`.
/// The low-level way to create a segment is to custom-make the bit buffer
/// and call the `QrSegment::new()` constructor with appropriate values.
///
/// This segment struct imposes no length restrictions, but QR Codes have restrictions.
/// Even in the most favorable conditions, a QR Code can only hold 7089 characters of data.
/// Any segment longer than this is meaningless for the purpose of generating QR Codes.
#[derive(Clone, PartialEq, Eq)]
pub struct QrSegment {
    /// The mode indicator of this segment.
    pub mode: QrSegmentMode,

    /// The length of this segment's unencoded data. Measured in characters for
    /// numeric/alphanumeric/kanji mode, bytes for byte mode, and 0 for ECI mode.
    /// Not the same as the data's bit length.
    pub num_chars: usize,

    /// The data bits of this segment.
    pub data: Vec<bool>,
}

impl QrSegment {
    /*---- Static factory functions (mid level) ----*/

    /// Returns a segment representing the given binary data encoded in byte mode.
    ///
    /// All input byte slices are acceptable.
    ///
    /// Any text string can be converted to UTF-8 bytes and encoded as a byte mode segment.
    pub fn make_bytes(data: &[u8]) -> Self {
        let mut bb = BitBuffer(Vec::with_capacity(data.len().checked_mul(8).unwrap()));
        for &b in data {
            bb.append_bits(u32::from(b), 8);
        }
        QrSegment::new(Byte, data.len(), bb.0)
    }

    /// Returns a segment representing the given string of decimal digits encoded in numeric mode.
    ///
    /// Panics if the string contains non-digit characters.
    pub fn make_numeric(text: &str) -> Self {
        assert!(
            text.bytes().all(|b| b.is_ascii_digit()),
            "String contains non-numeric characters"
        );
        let mut bb = BitBuffer(Vec::with_capacity(
            text.len()
                .checked_mul(3)
                .unwrap()
                .checked_add(text.len().div_ceil(3))
                .unwrap(),
        ));
        for chunk in text.as_bytes().chunks(3) {
            let data: u32 = chunk.iter().fold(0u32, |acc, &b| acc * 10 + u32::from(b - b'0'));
            bb.append_bits(data, (chunk.len() as u8) * 3 + 1);
        }
        QrSegment::new(Numeric, text.len(), bb.0)
    }

    /// Returns a segment representing the given text string encoded in alphanumeric mode.
    ///
    /// The characters allowed are: 0 to 9, A to Z (uppercase only), space,
    /// dollar, percent, asterisk, plus, hyphen, period, slash, colon.
    ///
    /// Panics if the string contains non-encodable characters.
    pub fn make_alphanumeric(text: &str) -> Self {
        let mut bb = BitBuffer(Vec::with_capacity(
            text.len()
                .checked_mul(5)
                .unwrap()
                .checked_add(text.len().div_ceil(2))
                .unwrap(),
        ));
        for chunk in text.as_bytes().chunks(2) {
            let data: u32 = chunk.iter().fold(0u32, |acc, &b| {
                acc * 45
                    + u32::try_from(
                        ALPHANUMERIC_CHARSET
                            .find(char::from(b))
                            .expect("String contains unencodable characters in alphanumeric mode"),
                    )
                    .unwrap()
            });
            bb.append_bits(data, (chunk.len() as u8) * 5 + 1);
        }
        QrSegment::new(Alphanumeric, text.len(), bb.0)
    }

    /// Returns a list of zero or more segments to represent the given Unicode text string.
    ///
    /// The result may use various segment modes and switch
    /// modes to optimize the length of the bit stream.
    pub fn make_segments(text: &str) -> Vec<Self> {
        if text.is_empty() {
            vec![]
        } else {
            vec![if QrSegment::is_numeric(text) {
                QrSegment::make_numeric(text)
            } else if QrSegment::is_alphanumeric(text) {
                QrSegment::make_alphanumeric(text)
            } else {
                QrSegment::make_bytes(text.as_bytes())
            }]
        }
    }

    /// Returns a segment representing an Extended Channel Interpretation
    /// (ECI) designator with the given assignment value.
    pub fn make_eci(assign_val: u32) -> Self {
        let mut bb = BitBuffer(Vec::with_capacity(24));
        if assign_val < (1 << 7) {
            bb.append_bits(assign_val, 8);
        } else if assign_val < (1 << 14) {
            bb.append_bits(0b10, 2);
            bb.append_bits(assign_val, 14);
        } else if assign_val < 1_000_000 {
            bb.append_bits(0b110, 3);
            bb.append_bits(assign_val, 21);
        } else {
            panic!("ECI assignment value out of range");
        }
        QrSegment::new(Eci, 0, bb.0)
    }

    /*---- Constructor (low level) ----*/

    /// Creates a new QR Code segment with the given attributes and data.
    ///
    /// The character count (numchars) must agree with the mode and
    /// the bit buffer length, but the constraint isn't checked.
    pub fn new(mode: QrSegmentMode, num_chars: usize, data: Vec<bool>) -> Self {
        Self { mode, num_chars, data }
    }

    /*---- Other static functions ----*/

    /// Calculates and returns the number of bits needed to encode the given
    /// segments at the given version. The result is None if a segment has too many
    /// characters to fit its length field, or the total bits exceeds usize::MAX.
    pub(crate) fn get_total_bits(segments: &[Self], version: Version) -> Option<usize> {
        let mut result: usize = 0;
        for seg in segments {
            let ccbits: u8 = seg.mode.num_char_count_bits(version);
            // ccbits can be as large as 16, but usize can be as small as 16
            if let Some(limit) = 1usize.checked_shl(ccbits.into()) {
                if seg.num_chars >= limit {
                    return None; // The segment's length doesn't fit the field's bit width
                }
            }
            result = result.checked_add(4 + usize::from(ccbits))?;
            result = result.checked_add(seg.data.len())?;
        }
        Some(result)
    }

    /// Tests whether the given string can be encoded as a segment in numeric mode.
    ///
    /// A string is encodable iff each character is in the range 0 to 9.
    pub fn is_numeric(text: &str) -> bool {
        text.chars().all(|c| ('0'..='9').contains(&c))
    }

    /// Tests whether the given string can be encoded as a segment in alphanumeric mode.
    ///
    /// A string is encodable iff each character is in the following set: `0` to `9`, `A` to `Z`
    /// (uppercase only), space, dollar, percent, asterisk, plus, hyphen, period, slash, colon.
    pub fn is_alphanumeric(text: &str) -> bool {
        text.chars().all(|c| ALPHANUMERIC_CHARSET.contains(c))
    }
}

impl QrSegmentMode {
    /// Returns an unsigned 4-bit integer value (range 0 to 15)
    /// representing the mode indicator bits for this mode object.
    pub(crate) fn mode_bits(self) -> u32 {
        use QrSegmentMode::*;
        match self {
            Numeric => 0x1,
            Alphanumeric => 0x2,
            Byte => 0x4,
            Kanji => 0x8,
            Eci => 0x7,
        }
    }

    /// Returns the bit width of the character count field for a segment in this mode
    /// in a QR Code at the given version number. The result is in the range [0, 16].
    pub(crate) fn num_char_count_bits(self, ver: Version) -> u8 {
        use QrSegmentMode::*;
        (match self {
            Numeric => [10, 12, 14],
            Alphanumeric => [9, 11, 13],
            Byte => [8, 16, 16],
            Kanji => [8, 10, 12],
            Eci => [0, 0, 0],
        })[usize::from((ver.value() + 7) / 17)]
    }
}
