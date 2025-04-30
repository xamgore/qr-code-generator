/// Maps the Unicode character to the index of the same character
/// at the [Shift JIS](https://en.wikipedia.org/wiki/Shift_JIS) encoding.
///
/// It is compatible to the original assignments of JIS X 0201 (`[21-7E A1-DF]`).
///
/// The 94 by 94 region of JIS X 0208 is sliced, or rather "shifted" into
/// the odd half (odd row number) and even half (even row number),
/// and merged into the 188 by 47 region mapped to `[81-9F E0-EF] [40-7E 80-FC]`.
///
/// The remaining area, `[80 A0 F0-FF] [40-7E 80-FC]`, has been subjected to
/// numerous extensions incompatible to each other.
///
/// This particular implementation uses IBM/NEC extensions which assigns more characters
/// to `[F0-FC 80-FC]` and also to the Private Use Area (PUA). It requires some cares to handle
/// since the second byte of JIS X 0208 can have its MSB unset.
pub fn unicode_to_jis(ch: char) -> Option<u16> {
    Some(match ch {
        '\u{0}'..='\u{7f}' => ch as u16, // as in ASCII
        '\u{a5}' => 0x5C,                // ¥ sign
        '\u{203e}' => 0x7E,              // overline
        '\u{ff61}'..='\u{ff9f}' => ((ch as u32 - 0xFF61 + 0xA1) as u8) as u16,
        _ => match encoding_index_japanese::jis0208::backward_remapped(ch as u32) {
            0xffff => return None,
            ptr => {
                let lead = ptr / 188;
                let lead_offset = if lead < 0x1F { 0x81 } else { 0xC1 };
                let trail = ptr % 188;
                let trail_offset = if trail < 0x3F { 0x40 } else { 0x41 };
                u16::from_be_bytes([(lead + lead_offset) as u8, (trail + trail_offset) as u8])
            }
        },
    })
}

/// In the [Shift JIS](https://en.wikipedia.org/wiki/Shift_JIS) encoding, Kanji characters
/// are represented by a two byte combination. Only a subset of them gets compacted
/// into 13-bit binary codewords.
pub fn jis_to_index(ch: u16) -> Option<u16> {
    let bytes = match ch {
        c @ 0x8140..=0x9FFC => c - 0x8140,
        c @ 0xE040..=0xEBBF => c - 0xC140,
        _ => return None, // other modes must be used
    };
    Some((bytes >> 8) * 0xc0 + (bytes & 0xff))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_to_jis() {
        assert_eq!(unicode_to_jis('点'), Some(0x935F));
        assert_eq!(unicode_to_jis('茗'), Some(0xE4AA));
    }

    #[test]
    fn test_unicode_kanji_to_jis() {
        assert_eq!(jis_to_index(0x935F), Some(0xD9F));
        assert_eq!(jis_to_index(0xE4AA), Some(0x1AAA));
    }
}
