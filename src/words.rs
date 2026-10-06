//! The word lists, compiled into the binary. Both files are sorted, lowercase, one
//! five-letter word per line; `tools/build-words.py` regenerates them from SCOWL.
//!
//! `definitions.txt` has one line per puzzle word, `word<TAB>meaning`, in the same
//! order. The meanings were written for this game, for children: short, plain, and the
//! sense worth learning. They are not taken from a dictionary.

use std::sync::OnceLock;

const ANSWERS: &str = include_str!("../words/answers.txt");
const ALLOWED: &str = include_str!("../words/allowed.txt");
const DEFINITIONS: &str = include_str!("../words/definitions.txt");
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

fn definitions() -> &'static [(&'static str, &'static str)] {
    static LIST: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    LIST.get_or_init(|| DEFINITIONS.lines().filter_map(|line| line.split_once('\t')).collect())
}

/// What a puzzle word means, in a few plain words. Case does not matter.
pub fn definition(word: &str) -> Option<&'static str> {
    let word = word.to_ascii_lowercase();
    let list = definitions();
    list.binary_search_by(|(w, _)| (*w).cmp(word.as_str())).ok().map(|at| list[at].1)
}

/// Whether a guess is in the dictionary. Case does not matter.
pub fn is_word(word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    allowed().binary_search(&word.as_str()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A definition is at most this long, so that it fits a dialog in three lines.
    const DEFINITION_MAX: usize = 70;

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
    fn every_answer_has_a_short_definition_and_nothing_else_does() {
        let defined: Vec<&str> = definitions().iter().map(|(w, _)| *w).collect();
        assert_eq!(defined, answers(), "definitions.txt must list exactly the words of answers.txt, in order");
        assert_eq!(definitions().len(), DEFINITIONS.lines().count(), "every line is word<TAB>meaning");
        for (word, meaning) in definitions() {
            let n = meaning.chars().count();
            assert!((3..=DEFINITION_MAX).contains(&n), "{word}: {n} characters");
            assert!(*meaning == meaning.trim() && !meaning.ends_with('.'), "{word}: stray space or full stop");
        }
        assert_eq!(definition("SKEIN"), Some("a loose bundle of yarn or thread"));
        assert_eq!(definition("zzzzz"), None);
    }

    #[test]
    fn lookup_ignores_case_and_rejects_non_words() {
        assert!(is_word("CRANE"));
        assert!(is_word("crane"));
        assert!(!is_word("zzzzz"));
        assert!(!is_word("cran"));
    }
}
