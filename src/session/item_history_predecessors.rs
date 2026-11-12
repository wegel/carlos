//! Find the preceding nonempty read shell with bounded machine-word work per edit.

// Each level marks nonempty words in the level below. Even a usize-sized history
// needs at most ceil(usize::BITS / 6) levels; edits never walk transcript rows.
const LEVELS: usize = (usize::BITS as usize).div_ceil(6);

pub(super) struct Predecessors {
    last: Option<usize>,
    words: [Vec<u64>; LEVELS],
    #[cfg(test)]
    pub(super) steps: usize,
}

impl Predecessors {
    pub(super) fn new() -> Self {
        Self {
            last: None,
            words: std::array::from_fn(|_| Vec::new()),
            #[cfg(test)]
            steps: 0,
        }
    }

    pub(super) fn set(&mut self, mut index: usize, present: bool) {
        if present && self.last.is_none_or(|last| index > last) {
            self.last = Some(index);
        } else if !present && self.last == Some(index) {
            self.last = self.before_at(0, index);
        }
        for words in &mut self.words {
            #[cfg(test)]
            {
                self.steps += 1;
            }
            let word = index / 64;
            if words.len() <= word {
                words.resize(word + 1, 0);
            }
            let old = words[word];
            let mask = 1 << (index % 64);
            if present {
                words[word] |= mask;
            } else {
                words[word] &= !mask;
            }
            if (old == 0) == (words[word] == 0) {
                break;
            }
            index = word;
        }
    }

    pub(super) fn before(&mut self, index: usize) -> Option<usize> {
        if self.last.is_none_or(|last| last < index) {
            #[cfg(test)]
            {
                self.steps += 1;
            }
            return self.last;
        }
        self.before_at(0, index)
    }

    fn before_at(&mut self, level: usize, before: usize) -> Option<usize> {
        if before == 0 || level == LEVELS {
            return None;
        }
        #[cfg(test)]
        {
            self.steps += 1;
        }
        let index = before - 1;
        let word = index / 64;
        let bits =
            self.words[level].get(word).copied().unwrap_or(0) & (u64::MAX >> (63 - index % 64));
        if bits != 0 {
            return Some(word * 64 + 63 - bits.leading_zeros() as usize);
        }
        let previous_word = self.before_at(level + 1, word)?;
        let bits = self.words[level][previous_word];
        Some(previous_word * 64 + 63 - bits.leading_zeros() as usize)
    }
}
