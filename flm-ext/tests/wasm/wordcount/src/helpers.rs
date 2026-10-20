//! The word count of case 1, the twin of `tests/js/wordcount/helpers.js`.

/// The words of `text` (its runs of characters between white space) that have at least
/// `min` characters.
pub fn count_words(text: &str, min: usize) -> usize {
    text.split_whitespace().filter(|word| word.chars().count() >= min).count()
}
