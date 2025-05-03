mod alphanumeric_to_idx;
pub use alphanumeric_to_idx::*;

mod unicode_kanji_to_idx;
pub use unicode_kanji_to_idx::*;

#[cfg(feature = "encoding-next-index-japanese")]
mod unicode_to_jis;
