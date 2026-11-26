// A language of the library highlight.js, which the package ships without a declaration of
// its own: the function that `registerLanguage` takes.

declare module 'highlight.js/lib/languages/*' {
  import type { LanguageFn } from 'highlight.js';
  const language: LanguageFn;
  export default language;
}
