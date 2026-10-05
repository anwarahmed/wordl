//! The word lists, compiled into the binary. Both files are sorted, lowercase, one
//! five-letter word per line; `tools/build-words.py` regenerates them from SCOWL.

use std::sync::OnceLock;

const ANSWERS: &str = include_str!("../words/answers.txt");
const ALLOWED: &str = include_str!("../words/allowed.txt");
/// The notice SCOWL asks to travel with every copy of its lists.
pub const SCOWL_NOTICE: &str = include_str!("../words/SCOWL-COPYRIGHT");

/// Words a puzzle can be.
pub fn answers() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| ANSWERS.lines().collect())
}

fn allowed() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| ALLOWED.lines().collect())
}

/// Whether a guess is in the dictionary. Case does not matter.
pub fn is_word(word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    allowed().binary_search(&word.as_str()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn well_formed(list: &[&str]) {
        assert!(list.iter().all(|w| w.len() == 5 && w.bytes().all(|b| b.is_ascii_lowercase())), "only five lowercase letters");
        assert!(list.windows(2).all(|p| p[0] < p[1]), "sorted, no duplicates");
    }

    #[test]
    fn lists_are_sorted_five_letter_words() {
        well_formed(answers());
        well_formed(allowed());
        assert!(answers().len() >= 1000);
    }

    #[test]
    fn every_answer_is_an_accepted_guess() {
        assert!(answers().iter().all(|w| is_word(w)));
    }

    #[test]
    fn lookup_ignores_case_and_rejects_non_words() {
        assert!(is_word("CRANE"));
        assert!(is_word("crane"));
        assert!(!is_word("zzzzz"));
        assert!(!is_word("cran"));
    }
}
