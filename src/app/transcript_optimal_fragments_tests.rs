//! Verify exceptional word fields survive conversion to the compact wide fallback.

use textwrap::core::Fragment;

use super::{SourceWord, Words};

#[test]
fn wide_fields_keep_existing_offsets_and_widths_without_a_word_or_body_cap() {
    let Ok(huge) = usize::try_from(u64::from(u32::MAX) + 1) else {
        return;
    };
    for (whitespace, width) in [(huge, 9), (1, huge)] {
        let mut records = Words::Compact(Vec::new());
        records.push(6, 1, 5);
        records.push(11, whitespace, width);
        records.push(15, 0, 4);
        let Words::Wide(words) = records else {
            panic!("exceptional fields need wide records");
        };
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].end(), 6);
        assert_eq!(words[0].width(), 5.0);
        assert_eq!(words[0].whitespace_bytes(), 1);
        assert_eq!(words[1].end(), 11);
        assert_eq!(words[1].width(), width as f64);
        assert_eq!(words[1].whitespace_bytes(), whitespace);
        assert_eq!(words[1].whitespace_width(), whitespace as f64);
        assert_eq!(words[2].end(), 15);
        assert_eq!(words[2].width(), 4.0);
    }
}
