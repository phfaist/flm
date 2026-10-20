//! The template engine that the feature `compose` contributes, [`ContentOnlyEngine`]
//! (`content-only`).
//!
//! A template engine is the code that applies a compose template written in its language
//! to what the compose template reads ([`TemplateEngine`]); a compose template refers to
//! its template engine by name. A host contributes the engines of the template languages
//! it reads.

use super::{Template, TemplateContext, TemplateEngine};

/// The name of [`ContentOnlyEngine`], `content-only`.
pub const CONTENT_ONLY: &str = "content-only";

/// The template engine `content-only`: it writes the pieces placed before the content, the
/// content, and the pieces placed after it, each beginning on a line of its own, with
/// nothing around them.
///
/// It reads no text of the compose template. An empty part is left out, and a result that
/// is not empty ends with a line break.
#[derive(Debug, Default, Clone, Copy)]
pub struct ContentOnlyEngine;

impl TemplateEngine for ContentOnlyEngine {
    fn name(&self) -> &str {
        CONTENT_ONLY
    }

    fn render(&self, _template: &Template, context: &TemplateContext) -> Result<String, String> {
        fn at<'c>(context: &'c TemplateContext, place: &'static str) -> impl Iterator<Item = &'c str> + 'c {
            context.pieces.iter().filter(move |piece| piece.place == place).map(|piece| piece.text.as_str())
        }
        let content = context
            .regions
            .iter()
            .find(|(name, _)| name == crate::core::region::CONTENT)
            .map(|(_, text)| text.as_str());
        let parts: Vec<&str> = at(context, "before-content")
            .chain(content)
            .chain(at(context, "after-content"))
            .filter(|part| !part.is_empty())
            .collect();
        let mut out = String::new();
        for part in parts {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(part);
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        Ok(out)
    }
}
