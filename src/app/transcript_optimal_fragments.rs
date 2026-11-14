//! Feed compact source offsets to textwrap's unchanged optimal-fit engine.

use textwrap::core::Fragment;
use textwrap::word_splitters::split_words;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::{Options, WrapAlgorithm};

const COMPACT_FROM_BYTES: usize = 16 * 1024;

/// Keep the exact fields the fixed no-hyphenation wrapper needs from upstream Word.
#[derive(Debug)]
pub(super) struct CompactWord {
    end: usize,
    whitespace: u32,
    width: u32,
}

/// Retain exceptional field values without reverting to the upstream fat string records.
#[derive(Debug)]
pub(super) struct WideWord {
    end: usize,
    whitespace: usize,
    width: usize,
}

enum Words {
    Compact(Vec<CompactWord>),
    Wide(Vec<WideWord>),
}

trait SourceWord: Fragment {
    fn end(&self) -> usize;
    fn whitespace_bytes(&self) -> usize;
}

impl Fragment for CompactWord {
    fn width(&self) -> f64 {
        self.width as f64
    }
    fn whitespace_width(&self) -> f64 {
        self.whitespace as f64
    }
    fn penalty_width(&self) -> f64 {
        0.0
    }
}

impl Fragment for WideWord {
    fn width(&self) -> f64 {
        self.width as f64
    }
    fn whitespace_width(&self) -> f64 {
        self.whitespace as f64
    }
    fn penalty_width(&self) -> f64 {
        0.0
    }
}

impl SourceWord for CompactWord {
    fn end(&self) -> usize {
        self.end
    }
    fn whitespace_bytes(&self) -> usize {
        self.whitespace as usize
    }
}

impl SourceWord for WideWord {
    fn end(&self) -> usize {
        self.end
    }
    fn whitespace_bytes(&self) -> usize {
        self.whitespace
    }
}

impl Words {
    fn push(&mut self, end: usize, whitespace: usize, width: usize) {
        match self {
            Self::Wide(words) => words.push(WideWord {
                end,
                whitespace,
                width,
            }),
            Self::Compact(words) => {
                if let (Ok(whitespace), Ok(width)) =
                    (u32::try_from(whitespace), u32::try_from(width))
                {
                    words.push(CompactWord {
                        end,
                        whitespace,
                        width,
                    });
                    return;
                }
                let mut wide = Vec::with_capacity(words.len() + 1);
                for word in std::mem::take(words) {
                    wide.push(WideWord {
                        end: word.end,
                        whitespace: word.whitespace as usize,
                        width: word.width as usize,
                    });
                }
                wide.push(WideWord {
                    end,
                    whitespace,
                    width,
                });
                *self = Self::Wide(wide);
            }
        }
    }

    fn shrink_to_fit(&mut self) {
        match self {
            Self::Compact(words) => words.shrink_to_fit(),
            Self::Wide(words) => words.shrink_to_fit(),
        }
    }
}

/// Preserve default optimal breaks without retaining upstream's three string references per word.
pub(super) fn visit_pieces(
    text: &str,
    options: &Options<'_>,
    mut visit: impl FnMut(&str, bool),
) -> bool {
    // Keep bounded ordinary batches on their existing allocation path.
    if text.len() <= COMPACT_FROM_BYTES {
        return false;
    }
    let WrapAlgorithm::OptimalFit(penalties) = &options.wrap_algorithm else {
        return false;
    };
    if text.contains('\n') {
        return false;
    }
    let Some(words) = collect_words(text, options) else {
        return false;
    };
    let widths = [options.width as f64, options.width as f64];
    match &words {
        Words::Compact(words) => visit_lines(words, &widths, penalties, text, &mut visit),
        Words::Wide(words) => visit_lines(words, &widths, penalties, text, &mut visit),
    }
    true
}

fn visit_lines<T: SourceWord>(
    words: &[T],
    widths: &[f64],
    penalties: &Penalties,
    text: &str,
    visit: &mut impl FnMut(&str, bool),
) {
    let lines = wrap_optimal_fit(words, widths, penalties)
        .expect("upstream optimal-fit does not overflow for usize line widths");
    let mut from = 0;
    for (index, line) in lines.iter().enumerate() {
        let last = index + 1 == lines.len();
        if let Some(word) = line.last() {
            visit(&text[from..word.end() - word.whitespace_bytes()], last);
            from = word.end();
        } else {
            visit("", last);
        }
    }
    debug_assert_eq!(from, text.len());
}

fn collect_words(text: &str, options: &Options<'_>) -> Option<Words> {
    let mut end = 0;
    let mut compact = Words::Compact(Vec::new());
    let words = options.word_separator.find_words(text);
    for word in split_words(words, &options.word_splitter) {
        if !word.penalty.is_empty() {
            return None;
        }
        end += word.word.len() + word.whitespace.len();
        compact.push(end, word.whitespace.len(), word.width);
    }
    compact.shrink_to_fit();
    Some(compact)
}

#[cfg(test)]
#[path = "transcript_optimal_fragments_tests.rs"]
mod tests;
