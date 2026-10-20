//! Image files on the host (cargo feature `graphics`): where an image's file is looked for
//! and that the reads are listed, what the inspector reads of each format, the in-process
//! converter, and the settings of the feature `graphics-files` and of the program's
//! defaults for images. No test touches the disk: the files are held in memory, the images
//! made by the tests.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::lang::FlmOrigin;
use flm_core::stdcore::backends::pres::{I48F16, Length, LengthUnit};
use flm_core::stdcore::features::graphics::{
    ConversionOptions, ConversionRequest, Dimensions, GraphicsConverter, GraphicsInspectRequest,
    GraphicsInspectedInformation, GraphicsInspector, INFO, ImageFile, ImageFormat, PixelSize, ResolvedImage,
};
use flm_main::files::{FileRead, MemoryFiles};
use flm_main::graphics::{FileInspector, RasterConverter};
use flm_main::output::{MemorySink, Naming, Sink};
use flm_main::session::{Built, Input, Prepared, Request, build, prepare};
use flm_main::settings::CommandLineSetting;
use flm_main::stdpkg::{self, StdParts};

// --- images made by the tests --------------------------------------------------------------

/// A PNG image of `width` × `height` gray pixels, stating `per_meter` dots per meter in each
/// direction where given.
fn png(width: u32, height: u32, per_meter: Option<(u32, u32)>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_pixel_dims(
        per_meter.map(|(xppu, yppu)| png::PixelDimensions { xppu, yppu, unit: png::Unit::Meter }),
    );
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&vec![128; (width * height) as usize]).unwrap();
    writer.finish().unwrap();
    bytes
}

/// `width` × `height` gray pixels, for the crate's encoders.
fn gray(width: u32, height: u32) -> image::DynamicImage {
    image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(width, height, image::Luma([128])))
}

/// A JPEG image whose JFIF header states `dpi` dots per inch.
fn jpeg(width: u32, height: u32, dpi: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut bytes);
    encoder.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(dpi));
    encoder.encode_image(&gray(width, height)).unwrap();
    bytes
}

/// A JPEG image with no JFIF header, whose EXIF data states `dpi` dots per inch.
fn jpeg_with_exif(width: u32, height: u32, dpi: u32) -> Vec<u8> {
    let plain = jpeg(width, height, 1);
    // The JFIF segment, which follows the start-of-image marker, is left out.
    let jfif_length = usize::from(u16::from_be_bytes([plain[4], plain[5]]));
    let rest = &plain[4 + jfif_length..];
    // A little-endian TIFF structure with one directory of two entries: the horizontal
    // resolution (a fraction stored after the directory) and its unit, inches.
    let mut tiff = b"II*\0".to_vec();
    tiff.extend(8u32.to_le_bytes());
    tiff.extend(2u16.to_le_bytes());
    tiff.extend(0x011Au16.to_le_bytes());
    tiff.extend(5u16.to_le_bytes());
    tiff.extend(1u32.to_le_bytes());
    tiff.extend(38u32.to_le_bytes());
    tiff.extend(0x0128u16.to_le_bytes());
    tiff.extend(3u16.to_le_bytes());
    tiff.extend(1u32.to_le_bytes());
    tiff.extend(2u32.to_le_bytes());
    tiff.extend(0u32.to_le_bytes());
    tiff.extend(dpi.to_le_bytes());
    tiff.extend(1u32.to_le_bytes());
    let mut segment = b"Exif\0\0".to_vec();
    segment.extend(tiff);
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1];
    bytes.extend(u16::try_from(segment.len() + 2).unwrap().to_be_bytes());
    bytes.extend(segment);
    bytes.extend(rest);
    bytes
}

/// An image in `format`, by the crate's encoder.
fn encoded(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    let picture = match format {
        // The WebP and GIF encoders take colors.
        image::ImageFormat::WebP | image::ImageFormat::Gif => {
            image::DynamicImage::ImageRgba8(gray(width, height).to_rgba8())
        }
        _ => gray(width, height),
    };
    picture.write_to(&mut Cursor::new(&mut bytes), format).unwrap();
    bytes
}

/// A PDF file of one page whose media box, `[0 0 200 100]`, is inherited from the page
/// tree, and whose user unit is 2.
fn pdf() -> Vec<u8> {
    pdf_of(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 100] >>",
        "<< /Type /Page /Parent 2 0 R /UserUnit 2 >>",
    ])
}

/// A PDF file holding `objects`, numbered from 1, the first one its catalog.
fn pdf_of(objects: &[&str]) -> Vec<u8> {
    let mut text = String::from("%PDF-1.6\n");
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(text.len());
        text.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = text.len();
    text.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for offset in offsets {
        text.push_str(&format!("{offset:010} 00000 n \n"));
    }
    text.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1));
    text.into_bytes()
}

// --- the host, over files in memory ---------------------------------------------------------

/// A request for `doc/paper.flm`, producing `format`.
fn request(format: &str) -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from(format));
    request
}

fn prepared(files: MemoryFiles, request: Request) -> Prepared {
    prepare(request, Arc::new(files), &[]).unwrap_or_else(|error| panic!("{error:?}"))
}

fn built(files: MemoryFiles, request: Request) -> Built {
    build(&prepared(files, request)).unwrap_or_else(|error| panic!("{error:?}"))
}

/// The identities of the files the resolver found, in the document's order.
fn identities(built: &Built) -> Vec<String> {
    let info = built.run.table(INFO).unwrap().expect("the record table");
    info.iter().filter_map(|(_, record)| record.file.as_ref().map(|file| file.identity.clone())).collect()
}

/// The reads of `built` at `path`.
fn reads_of<'b>(built: &'b Built, path: &str) -> Vec<&'b FileRead> {
    built.files_read.iter().filter(|read| read.path == Path::new(path)).collect()
}

/// A document whose images may stand anywhere, with the lines `front` added to the settings
/// of its front matter: at the level of a feature's name, or, indented by two more spaces,
/// among the keys of the feature `graphics`.
fn document(front: &str, content: &str) -> String {
    format!("---\nflm:\n  features:\n    graphics:\n      global: true\n{front}---\n{content}\n")
}

// --- acceptance 1: where a file is looked for, and the list of files read ---------------------

#[test]
fn a_path_is_found_in_the_first_folder_of_the_search_path_that_holds_it() {
    let front = "    graphics-files:\n      search_path: [figures, art, .]\n";
    let files = MemoryFiles::new()
        .with("doc/paper.flm", &document(front, "A \\includegraphics{plot.png}."))
        .with("doc/art/plot.png", "")
        .with("doc/plot.png", "");
    let built = built(files, request("text"));
    assert_eq!(identities(&built), ["doc/art/plot.png"]);
    // The folder searched first and holding no such file is listed as not found.
    assert!(reads_of(&built, "doc/figures/plot.png").iter().all(|read| !read.found));
    assert!(!reads_of(&built, "doc/figures/plot.png").is_empty());
    assert!(reads_of(&built, "doc/art/plot.png").iter().all(|read| read.found));
    assert!(reads_of(&built, "doc/plot.png").is_empty());
}

#[test]
fn with_no_search_path_set_a_path_is_found_in_the_folder_of_the_file_naming_it() {
    let mut files = MemoryFiles::new().with("doc/paper.flm", &document("", "A \\includegraphics{figures/plot.png}."));
    files.insert("doc/figures/plot.png", png(1, 1, Some((3780, 3780))));
    let built = built(files, request("text"));
    assert_eq!(identities(&built), ["doc/figures/plot.png"]);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
}

#[test]
fn a_file_not_found_is_reported_and_listed() {
    let files = MemoryFiles::new().with("doc/paper.flm", &document("", "A \\includegraphics{missing.png}."));
    let built = built(files, request("text"));
    let error = built.diagnostics.iter().find(|d| d.severity() == flm_core::techy::error::Severity::Error);
    let printed = flm_main::report::diagnostic(error.expect("an error"));
    assert!(printed.contains("missing.png") && printed.contains("`doc`"), "{printed}");
    assert_eq!(reads_of(&built, "doc/missing.png").len(), 1);
    assert!(!reads_of(&built, "doc/missing.png")[0].found);
}

#[test]
fn an_image_in_an_included_file_is_looked_for_from_that_file_s_folder() {
    let files = MemoryFiles::new()
        .with("doc/paper.flm", &document("", "Intro.\n\n\\input{parts/one.flm}"))
        .with("doc/parts/one.flm", "See \\includegraphics{pic.png}.\n")
        .with("doc/parts/pic.png", "");
    let built = built(files, request("text"));
    assert_eq!(identities(&built), ["doc/parts/pic.png"]);
}

#[test]
fn each_build_of_one_prepared_work_lists_its_own_reads() {
    let files = MemoryFiles::new()
        .with("doc/paper.flm", &document("", "A \\includegraphics{plot.png}."))
        .with("doc/plot.png", "");
    let prepared = prepared(files, request("text"));
    let first = build(&prepared).unwrap();
    let second = build(&prepared).unwrap();
    assert_eq!(reads_of(&first, "doc/plot.png").len(), reads_of(&second, "doc/plot.png").len());
    assert!(!reads_of(&second, "doc/plot.png").is_empty());
    // What prepare read is the same snapshot at the head of both lists.
    let snapshot = prepared.files_read();
    assert_eq!(&first.files_read[..snapshot.len()], snapshot);
    assert_eq!(&second.files_read[..snapshot.len()], snapshot);
}

// --- acceptance 2: what the inspector reads of each format -----------------------------------

/// What the inspector answers about `content`, a file of the media type `media_type`.
fn inspect(content: Vec<u8>, media_type: &str) -> Result<GraphicsInspectedInformation, String> {
    let file = ImageFile::new("the-file", media_type, "the-file");
    let resolved = ResolvedImage { file, content: content.into() };
    let request = GraphicsInspectRequest::new("the-file", FlmOrigin::new("paper.flm")).resolved(resolved);
    FileInspector.inspect(&request).map_err(|error| error.detail)
}

fn raster(width: u32, height: u32, dpi: Option<f64>) -> GraphicsInspectedInformation {
    GraphicsInspectedInformation::Raster {
        dpi: dpi.map(I48F16::from_num),
        size: PixelSize { width, height },
        physical_dimensions: None,
    }
}

#[test]
fn the_inspector_reads_the_size_and_the_resolution_of_each_raster_format() {
    // 5000 dots per meter are 127 dots per inch.
    assert_eq!(inspect(png(3, 2, Some((5000, 5000))), "image/png"), Ok(raster(3, 2, Some(127.0))));
    assert_eq!(inspect(png(3, 2, None), "image/png"), Ok(raster(3, 2, None)));
    assert_eq!(inspect(jpeg(4, 5, 150), "image/jpeg"), Ok(raster(4, 5, Some(150.0))));
    assert_eq!(inspect(jpeg_with_exif(4, 5, 300), "image/jpeg"), Ok(raster(4, 5, Some(300.0))));
    assert_eq!(inspect(encoded(6, 7, image::ImageFormat::Gif), "image/gif"), Ok(raster(6, 7, None)));
    assert_eq!(inspect(encoded(8, 9, image::ImageFormat::WebP), "image/webp"), Ok(raster(8, 9, None)));
}

#[test]
fn a_resolution_differing_between_the_two_directions_is_refused() {
    let error = inspect(png(3, 2, Some((5000, 2500))), "image/png").unwrap_err();
    assert!(error.contains("must be equal"), "{error}");
}

#[test]
fn the_inspector_reads_the_size_an_svg_drawing_states() {
    let svg = br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="4in" height="96px"/>"#;
    let expected = GraphicsInspectedInformation::Vector {
        physical_dimensions: Dimensions {
            width: Length::new(I48F16::from_num(4), LengthUnit::Inches),
            height: Length::new(I48F16::from_num(72), LengthUnit::Points),
        },
    };
    assert_eq!(inspect(svg.to_vec(), "image/svg+xml"), Ok(expected));
    let no_size = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;
    assert!(inspect(no_size.to_vec(), "image/svg+xml").unwrap_err().contains("`width`"));
}

#[test]
fn the_inspector_reads_the_first_page_of_a_pdf_file() {
    let expected = GraphicsInspectedInformation::Vector {
        physical_dimensions: Dimensions {
            width: Length::new(I48F16::from_num(400), LengthUnit::Points),
            height: Length::new(I48F16::from_num(200), LengthUnit::Points),
        },
    };
    assert_eq!(inspect(pdf(), "application/pdf"), Ok(expected));
}

#[test]
fn a_pdf_page_tree_whose_parents_loop_is_refused() {
    // The first page states no media box, and the page tree above it names itself as its
    // parent: the climb looking for an inherited media box must end, with an error.
    let looping = pdf_of(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /Parent 2 0 R >>",
        "<< /Type /Page /Parent 2 0 R >>",
    ]);
    assert!(inspect(looping, "application/pdf").is_err());
}

#[test]
fn a_file_of_no_known_format_is_refused() {
    assert!(inspect(b"hello".to_vec(), "application/octet-stream").is_err());
}

// --- acceptance 3: the in-process converter ------------------------------------------------

#[test]
fn a_gif_made_a_png_has_the_same_size_in_pixels() {
    let gif = encoded(5, 3, image::ImageFormat::Gif);
    assert!(RasterConverter.offers(ImageFormat::Gif, ImageFormat::Png));
    assert!(!RasterConverter.offers(ImageFormat::Pdf, ImageFormat::Png));
    let options = ConversionOptions::new();
    let converted =
        RasterConverter.convert(&ConversionRequest::new(&gif, ImageFormat::Gif, ImageFormat::Png, &options)).unwrap();
    assert_eq!(image::guess_format(&converted).unwrap(), image::ImageFormat::Png);
    let picture = image::load_from_memory(&converted).unwrap();
    assert_eq!((picture.width(), picture.height()), (5, 3));
}

// --- acceptance 5: the settings --------------------------------------------------------------

/// Builds `request` over `files` into a `MemorySink`, and gives the files it received and
/// the text of the entry.
fn rendered(files: MemoryFiles, mut request: Request) -> (Vec<String>, String) {
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    request.naming = Naming::File { name: String::from("paper.html") };
    let built = built(files, request);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let received = sink.files();
    let entry = received.values().find(|file| file.entry).expect("an output");
    (received.keys().cloned().collect(), String::from_utf8(entry.bytes.clone()).unwrap())
}

fn with_image(front: &str) -> MemoryFiles {
    let mut files = MemoryFiles::new().with("doc/paper.flm", &document(front, "A \\includegraphics{plot.png}."));
    files.insert("doc/plot.png", png(1, 1, Some((3780, 3780))));
    files
}

#[test]
fn the_search_path_is_a_setting_the_schema_lists() {
    let stdmain = stdpkg::stdmain_factory(StdParts::new(Arc::new(Naming::Folder))).unwrap();
    let stdcore = flm_core::stdcore::factory().unwrap();
    let schema = flm_main::seed::front_matter_schema(&[&stdcore, &stdmain], &[]);
    let text = schema.to_string();
    assert!(text.contains("search_path"), "{text}");
}

#[test]
fn a_strategy_written_for_one_output_leaves_the_program_s_default_for_the_page() {
    let (files, page) = rendered(with_image("      strategy:\n        latex-recomposed: copy\n"), request("html"));
    assert_eq!(files, ["paper.html"]);
    assert!(page.contains("src=\"data:image/png;base64,"), "{page}");
}

#[test]
fn a_strategy_given_on_the_command_line_sets_one_output_s() {
    let mut request = request("html");
    request.options.push(CommandLineSetting::new("graphics.strategy.html=copy", "-O graphics.strategy.html=copy"));
    let (files, page) = rendered(with_image(""), request);
    assert_eq!(files.len(), 2, "{files:?}");
    let copy = files.iter().find(|name| name.ends_with(".png")).expect("the copy");
    assert!(copy.starts_with("paper-plot-"), "{copy}");
    assert!(page.contains(&format!("src=\"{copy}\"")), "{page}");
}

