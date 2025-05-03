#![allow(dead_code)]
#![allow(unused_variables)]

pub fn unicode_kanji_to_idx(ch: char) -> Option<u16> {
    #[cfg(feature = "encoding-next-index-japanese")]
    {
        super::unicode_to_jis::unicode_to_jis(ch).and_then(jis_to_index)
    }
    #[cfg(not(feature = "encoding-next-index-japanese"))]
    None
}

/// In the [Shift JIS](https://en.wikipedia.org/wiki/Shift_JIS) encoding, Kanji characters
/// are represented by a two byte combination. Only a subset of them gets compacted
/// into 13-bit binary codewords.
fn jis_to_index(ch: u16) -> Option<u16> {
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
    fn test_unicode_kanji_to_jis() {
        assert_eq!(jis_to_index(0x935F), Some(0xD9F));
        assert_eq!(jis_to_index(0xE4AA), Some(0x1AAA));
    }
}
