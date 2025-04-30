use QrSegmentMode::*;

use crate::bit_buffer::BitBuffer;
use crate::correction_code::ErrCorrectLvl;
use crate::error::DataTooLong;
use crate::helpers::{alphanumeric_to_idx, jis_to_index, unicode_to_jis};
use crate::prelude::QrCode;
use crate::version::Version;

/// Describes how a segment's data bits are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QrSegment {
    /// The mode indicator of this segment.
    pub mode: QrSegmentMode,

    /// The length of this segment's unencoded data. Measured in characters for
    /// numeric/alphanumeric/kanji mode, bytes for byte mode, and 0 for ECI mode.
    /// Not the same as the data's bit length.
    pub num_chars: usize,

    /// The data bits of this segment.
    pub data: BitBuffer,
}

impl QrSegment {
    /*---- Static factory functions (mid level) ----*/

    /// Returns a segment representing the given binary data encoded in byte mode.
    ///
    /// All input byte slices are acceptable.
    ///
    /// Any text string can be converted to UTF-8 bytes and encoded as a byte mode segment.
    pub fn make_bytes(data: &[u8]) -> Self {
        let cap = data.len().checked_mul(8).unwrap();
        let mut bb = BitBuffer::from(Vec::with_capacity(cap));
        for &b in data {
            bb.append_bits(u32::from(b), 8);
        }
        QrSegment::new(Byte, data.len(), bb)
    }

    /// Tests whether the given string can be encoded as a segment in numeric mode.
    ///
    /// A string is encodable iff each character is in the range 0 to 9.
    pub fn is_numeric(text: &str) -> bool {
        text.chars().all(|c| c.is_ascii_digit())
    }

    /// Returns a segment representing the given string of decimal digits encoded in numeric mode.
    ///
    /// Panics if the string contains non-digit characters.
    pub fn make_numeric(text: &str) -> Self {
        assert!(
            text.bytes().all(|b| b.is_ascii_digit()),
            "String contains non-numeric characters"
        );
        // text.len * 3.33(3)
        let capacity = text
            .len()
            .checked_mul(3)
            .unwrap()
            .checked_add(text.len().div_ceil(3))
            .unwrap();
        let mut bb = BitBuffer::from(Vec::with_capacity(capacity));
        for chunk in text.as_bytes().chunks(3) {
            let data: u32 = chunk.iter().fold(0u32, |acc, &b| acc * 10 + u32::from(b - b'0'));
            bb.append_bits(data, (chunk.len() as u8) * 3 + 1);
        }
        QrSegment::new(Numeric, text.len(), bb)
    }

    /// The set of all legal characters in alphanumeric mode,
    /// where each character value maps to the index in the string.
    const ALPHANUMERIC_CHARSET: &'static str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

    /// Tests whether the given string can be encoded as a segment in alphanumeric mode.
    ///
    /// A string is encodable iff each character is in the following set: `0` to `9`, `A` to `Z`
    /// (uppercase only), space, dollar, percent, asterisk, plus, hyphen, period, slash, colon.
    pub fn is_alphanumeric(text: &str) -> bool {
        text.chars().all(|c| Self::ALPHANUMERIC_CHARSET.contains(c))
    }

    /// Returns a segment representing the given text string encoded in alphanumeric mode.
    ///
    /// The characters allowed are: `0` to `9`, `A` to `Z` (uppercase only), space,
    /// dollar, percent, asterisk, plus, hyphen, period, slash, colon.
    ///
    /// Panics if the string contains non-encodable characters.
    pub fn make_alphanumeric(text: &str) -> Self {
        // text.len * 5.5
        let capacity = text
            .len()
            .checked_mul(5)
            .unwrap()
            .checked_add(text.len().div_ceil(2))
            .unwrap();
        let mut bb = BitBuffer::from(Vec::with_capacity(capacity));
        for chunk in text.as_bytes().chunks(2) {
            let data: u32 = chunk.iter().fold(0u32, |acc, &b| acc * 45 + alphanumeric_to_idx(b));
            bb.append_bits(data, (chunk.len() as u8) * 5 + 1);
        }
        QrSegment::new(Alphanumeric, text.len(), bb)
    }

    /// Returns a segment representing the specified text string encoded in kanji mode.
    ///
    /// The set of encodable characters: kanji used in Japan, hiragana, katakana,
    /// East Asian punctuation, full-width ASCII, Greek, Cyrillic.
    ///
    /// Non-encodable characters include: ordinary ASCII, half-width katakana, more extensive
    /// Chinese hanzi.
    pub fn make_kanji(text: &str) -> Self {
        let num_chars = text.chars().count();
        let capacity = num_chars * 13; // 13 bits per Shift JIS char
        let mut bb = BitBuffer::from(Vec::with_capacity(capacity));
        text.chars()
            .filter_map(unicode_to_jis)
            .filter_map(jis_to_index)
            .for_each(|ch| bb.append_bits(ch as u32, 13));
        QrSegment::new(Kanji, num_chars, bb)
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

    /// Returns a list of segments to represent the given text, where the overall bit length is minimal.
    pub fn make_compact_segments(
        text: &str,
        err_correct_lvl: ErrCorrectLvl,
        min_version: Version,
        max_version: Version,
    ) -> Result<Vec<Self>, DataTooLong> {
        let mut segments = Vec::new();
        let mut version = min_version;

        loop {
            if version == min_version || matches!(*version, 10 | 27) {
                segments = make_compact_segments(text, version).unwrap_or_default();
            }

            let data_capacity_bits: usize = QrCode::get_num_data_codewords(version, err_correct_lvl) * 8; // Number of data bits available
            let data_used: Option<usize> = QrSegment::get_total_bits(&segments, version);

            if data_used.is_some_and(|n| n <= data_capacity_bits) {
                return Ok(segments); // This version number is found to be suitable
            } else if version >= max_version {
                // All versions in the range could not fit the given data
                return Err(match data_used {
                    None => DataTooLong::SegmentTooLong,
                    Some(n) => DataTooLong::DataOverCapacity(n, data_capacity_bits),
                });
            } else {
                version += 1;
            }
        }
    }

    /// Returns a segment representing an Extended Channel Interpretation
    /// designator with the given assignment value. (See AIM ECI specification.)
    pub fn make_eci(val: u32) -> Self {
        let mut bb = BitBuffer::from(Vec::with_capacity(24));
        if val < (1 << 7) {
            bb.append_bits(val, 8);
        } else if val < (1 << 14) {
            bb.append_bits(0b10, 2);
            bb.append_bits(val, 14);
        } else if val < 1_000_000 {
            bb.append_bits(0b110, 3);
            bb.append_bits(val, 21);
        } else {
            panic!("ECI assignment value out of range");
        }
        QrSegment::new(Eci, 0, bb)
    }

    /*---- Constructor (low level) ----*/

    /// Creates a new QR Code segment with the given attributes and data.
    ///
    /// The character count (`num_chars`) must agree with the mode and
    /// the bit buffer length, but the constraint isn't checked.
    pub fn new(mode: QrSegmentMode, num_chars: usize, data: BitBuffer) -> Self {
        Self { mode, num_chars, data }
    }

    /*---- Other static functions ----*/

    /// Calculates and returns the number of bits needed to encode the given
    /// segments at the given version. The result is `None` if a segment has too many
    /// characters to fit its length field, or the total bits exceeds [usize::MAX].
    pub(crate) fn get_total_bits(segments: &[Self], version: Version) -> Option<usize> {
        let mut result: usize = 0;
        for seg in segments {
            let cc_bits: u8 = seg.mode.num_char_count_bits(version);
            // cc_bits can be as large as 16, but usize can be as small as 16
            if let Some(limit) = 1usize.checked_shl(cc_bits.into()) {
                if seg.num_chars >= limit {
                    return None; // The segment's length doesn't fit the field's bit width
                }
            }
            result = result.checked_add(4 + usize::from(cc_bits))?;
            result = result.checked_add(seg.data.len())?;
        }
        Some(result)
    }
}

impl QrSegmentMode {
    /// Returns an unsigned 4-bit integer value (range 0 to 15)
    /// representing **the mode indicator** bits for this mode object.
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
    /// in a QR Code at the given version number. The result is in the range `[0, 16]`.
    pub(crate) fn num_char_count_bits(self, ver: Version) -> u8 {
        use QrSegmentMode::*;
        let widths = match self {
            Numeric => [10, 12, 14],
            Alphanumeric => [9, 11, 13],
            Byte => [8, 16, 16],
            Kanji => [8, 10, 12],
            Eci => [0, 0, 0],
        };
        widths[(usize::from(ver) + 7) / 17]
    }
}

/// Returns a new list of segments that is optimal for the given text at the given version number.
fn make_compact_segments(text: &str, ver: Version) -> Result<Vec<QrSegment>, DataTooLong> {
    use QrSegmentMode::*;

    let text_size = text.chars().count();
    match text_size {
        0 => return Ok(Vec::new()),
        // upper bound is the number of characters that fit in QR Code version 40, low error correction, numeric mode
        // TODO: it's actually DataOverCapacity
        7090.. => return Err(DataTooLong::SegmentTooLong),
        _ => {}
    }

    const NUM_MODES: usize = 4;
    let mode_types: [QrSegmentMode; NUM_MODES] = [Byte, Alphanumeric, Numeric, Kanji]; // do not modify
    let head_costs = mode_types.map(|mode| 4 + 6 * mode.num_char_count_bits(ver) as usize);

    let mut char_modes: Vec<[Option<QrSegmentMode>; NUM_MODES]> = vec![[None; NUM_MODES]; text_size];
    let mut prev_costs = head_costs;

    for (i, c) in text.chars().enumerate() {
        let mut cur_costs = [0; NUM_MODES];

        // always extend a byte mode segment
        cur_costs[0] = prev_costs[0] + c.len_utf8() * 8 * 6;
        char_modes[i][0] = Some(mode_types[0]);

        // extend a segment if possible
        if QrSegment::ALPHANUMERIC_CHARSET.contains(c) {
            cur_costs[1] = prev_costs[1] + 33; // 5.5 bits per alphanumeric char
            char_modes[i][1] = Some(mode_types[1]);
        }
        if c.is_ascii_digit() {
            cur_costs[2] = prev_costs[2] + 20; // 3.33 bits per digit
            char_modes[i][2] = Some(mode_types[2]);
        }
        if unicode_to_jis(c).and_then(jis_to_index).is_some() {
            cur_costs[3] = prev_costs[3] + 78; // 13 bits per Shift JIS char
            char_modes[i][3] = Some(mode_types[3]);
        }

        // start new segment at the end to switch modes
        for j in 0..NUM_MODES {
            for k in 0..NUM_MODES {
                let new_cost = cur_costs[k].div_ceil(6) * 6 + head_costs[j];
                if char_modes[i][k].is_some() && (char_modes[i][j].is_none() || new_cost < cur_costs[j]) {
                    cur_costs[j] = new_cost;
                    char_modes[i][j] = Some(mode_types[k]);
                }
            }
        }

        // a non-tight upper bound is when each of 7089 characters switches to
        // byte mode (4-bit header + 16-bit count) and requires 4 bytes in UTF-8
        debug_assert!(cur_costs.iter().all(|&cost| cost <= (4 + 16 + 32) * 6 * 7089));

        prev_costs = cur_costs;
    }

    // find optimal ending mode
    let (_, mut cur_mode) = std::iter::zip(prev_costs, mode_types).min().unwrap();

    // get optimal mode for each code point by tracing backwards
    let mut optimal_modes = vec![Eci; text_size];

    for (i, res) in optimal_modes.iter_mut().enumerate().rev() {
        for j in 0..NUM_MODES {
            if mode_types[j] == cur_mode {
                cur_mode = char_modes[i][j].unwrap();
                *res = cur_mode;
                break;
            }
        }
    }

    merge_segments(text, optimal_modes)
}

fn merge_segments(text: &str, modes: Vec<QrSegmentMode>) -> Result<Vec<QrSegment>, DataTooLong> {
    let mut list = Vec::new();
    let mut offset = 0;

    for chunk in modes.chunk_by(|x, y| x == y) {
        let bytes: usize = text[offset..].chars().take(chunk.len()).map(|ch| ch.len_utf8()).sum();
        let text = &text[offset..(offset + bytes)];
        offset += bytes;

        let segment = match chunk[0] {
            Numeric => QrSegment::make_numeric(text),
            Alphanumeric => QrSegment::make_alphanumeric(text),
            Byte => QrSegment::make_bytes(text.as_bytes()),
            Kanji => QrSegment::make_kanji(text),
            Eci => unreachable!(),
        };

        list.push(segment)
    }

    Ok(list)
}
