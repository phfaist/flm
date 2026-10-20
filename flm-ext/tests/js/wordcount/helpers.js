// tests/js/wordcount/helpers.js: imported as "./helpers.js" by case 1's main module.
// The words of `text` that have at least `min_length` characters.
export function count_words(text, min_length) {
  return text.split(/\s+/).filter(word => word.length >= min_length).length;
}
