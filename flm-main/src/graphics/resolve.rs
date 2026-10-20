//! The host's graphics resolver: from the path of an image, as written in a document, to
//! the file that the path refers to, found in the folders of a search path and read
//! through [`crate::files`].

use std::path::{Path, PathBuf};

use flm_core::stdcore::features::graphics::{
    GraphicsResolveError, GraphicsResolveRequest, GraphicsResolver, ImageFile, ImageFormat,
};

use crate::files::{FileErrorKind, ServiceFiles, fold};
use crate::output::OutputFolder;

/// The media type given to a file whose format is none of the six image formats that the
/// library knows, `application/octet-stream`.
pub const UNKNOWN_MEDIA_TYPE: &str = "application/octet-stream";

/// The host's graphics resolver: it finds the file that an image's path refers to in the
/// folders of a search path, and reads it. See the [module
/// documentation](super#where-an-image-file-is-looked-for).
#[derive(Debug, Clone)]
pub struct FileResolver {
    files: ServiceFiles,
    search_path: Vec<PathBuf>,
    output: OutputFolder,
}

impl FileResolver {
    /// Returns the graphics resolver that reads through `files` and looks in each folder of
    /// `search_path`, in order, each relative to the folder of the file in which the image
    /// is written, or absolute. As the address at which an output reaches a file in its own
    /// folder, it gives the file's path as seen from the output's folder `output`.
    pub fn new(files: ServiceFiles, search_path: Vec<PathBuf>, output: OutputFolder) -> FileResolver {
        FileResolver { files, search_path, output }
    }
}

impl GraphicsResolver for FileResolver {
    fn resolve(&self, request: &GraphicsResolveRequest) -> Result<ImageFile, GraphicsResolveError> {
        // The folder of the file naming the image; the current folder, the empty path, for
        // a text given as the input (named `<input>`) and for a source with no path.
        let base = request.origin.canonical_path().and_then(|name| Path::new(name).parent()).unwrap_or(Path::new(""));
        let reader = self.files.reader();
        let mut searched = Vec::new();
        for folder in &self.search_path {
            let folder = base.join(folder);
            // Every path is read, and named, in the one spelling `.` and `..` folded away.
            let path = fold(&folder.join(&request.path));
            match reader.read_bytes(Path::new(""), &path) {
                Ok(content) => {
                    let media_type = media_type_of(&content, &path);
                    let address = self.output.address_of(&path);
                    return Ok(ImageFile::new(path.to_string_lossy(), media_type, address));
                }
                Err(error) if error.kind == FileErrorKind::NotFound => {
                    searched.push(format!("`{}`", display_folder(&fold(&folder))));
                }
                Err(error) => return Err(GraphicsResolveError::new(error.to_string())),
            }
        }
        Err(GraphicsResolveError::new(format!(
            "no file at this path exists in the folders searched: {}",
            searched.join(", ")
        )))
    }

    fn read(&self, file: &ImageFile) -> Result<Vec<u8>, GraphicsResolveError> {
        self.files
            .reader()
            .read_bytes(Path::new(""), Path::new(&file.identity))
            .map_err(|error| GraphicsResolveError::new(error.to_string()))
    }
}

/// Returns a folder as a message writes it: the current folder, the empty path, as `.`.
fn display_folder(folder: &Path) -> String {
    if folder.as_os_str().is_empty() { String::from(".") } else { folder.to_string_lossy().into_owned() }
}

/// Returns the media type of the file at `path` that holds `content`: told from the
/// content's first bytes where they are those of one of the six image formats that the
/// library knows, else from the extension of the file's name (`jpg` included), else
/// [`UNKNOWN_MEDIA_TYPE`].
pub fn media_type_of(content: &[u8], path: &Path) -> String {
    let format = format_of_content(content).or_else(|| {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "jpg" => Some(ImageFormat::Jpeg),
            other => ImageFormat::from_name(other),
        }
    });
    format.map_or_else(|| String::from(UNKNOWN_MEDIA_TYPE), |format| format.media_type().to_string())
}

/// Returns the format whose signature `content` begins with. An SVG drawing has no
/// signature: it is told by an `<svg` element within its first kilobyte, which may come
/// after an XML declaration and comments.
fn format_of_content(content: &[u8]) -> Option<ImageFormat> {
    if content.starts_with(b"%PDF-") {
        return Some(ImageFormat::Pdf);
    }
    match image::guess_format(content) {
        Ok(image::ImageFormat::Png) => return Some(ImageFormat::Png),
        Ok(image::ImageFormat::Jpeg) => return Some(ImageFormat::Jpeg),
        Ok(image::ImageFormat::Gif) => return Some(ImageFormat::Gif),
        Ok(image::ImageFormat::WebP) => return Some(ImageFormat::Webp),
        _ => {}
    }
    let head = &content[..content.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    let start = text.trim_start_matches('\u{feff}').trim_start();
    (start.starts_with('<') && text.contains("<svg")).then_some(ImageFormat::Svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_format_is_told_by_the_content_then_by_the_name() {
        assert_eq!(media_type_of(b"%PDF-1.4\n", Path::new("a.png")), "application/pdf");
        assert_eq!(media_type_of(b"<?xml version='1.0'?>\n<svg width='1'/>", Path::new("a")), "image/svg+xml");
        assert_eq!(media_type_of(b"", Path::new("a.JPG")), "image/jpeg");
        assert_eq!(media_type_of(b"", Path::new("a.svg")), "image/svg+xml");
        assert_eq!(media_type_of(b"hello", Path::new("a.txt")), UNKNOWN_MEDIA_TYPE);
    }
}
