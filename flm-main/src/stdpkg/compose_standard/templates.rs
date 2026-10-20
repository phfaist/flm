//! The compose templates of the package `stdmain`, read as an extension's are
//! ([`flm_ext::templates`]).
//!
//! `stdmain`'s compose templates are kept under `templates/OUTPUT/` in this crate's sources,
//! in the two forms of an extension's: the file `NAME.yaml`, a description alone, for a
//! compose template with no text (each `none`, whose template engine `content-only` reads
//! none), or the folder `NAME/` holding the description `flmtemplate.yaml`, whose key `main`
//! gives the template's text, a file of the folder. The compose template's name is the name
//! of its file without `.yaml`, or of its folder. A description is a map of the keys that
//! [`Template::from_data`] lists.
//!
//! The build script of this crate (`build.rs`) compiles every file of the folder into the
//! program, as a static table of paths and bytes, each path `templates/` followed by the
//! file's path in the folder (`templates/html/simple/flmtemplate.yaml`). [`std_templates`]
//! reads them from that table ([`StaticFiles`]) with the reader of an extension's compose
//! templates ([`flm_ext::templates::read_folder`]), each description through the YAML step
//! of an extension's data files. Each compose template may therefore include the files of
//! the folder ([`Template::files`]), as an extension's may.
//!
//! Adding a compose template to `stdmain` is writing its file or its folder. The outputs are
//! independent: a compose template may exist for one output only. `flm render --help`
//! prints the names of the compose templates by output under the option `-t`.

use flm_core::stdcore::features::compose::Template;
use flm_ext::engine::ExtensionIdentity;

use crate::extensions::StaticFiles;
use crate::stdpkg::STDMAIN_PACKAGE;

include!(concat!(env!("OUT_DIR"), "/std_templates.rs"));

/// Returns the compose templates of `stdmain` in the order of the table: by output, then by
/// the name of each one's file or folder, in name order. See the [module
/// documentation](self).
///
/// # Errors
///
/// Returns a text, for a person to read, that says why a compose template of `stdmain`
/// cannot be read, the problems the reader finds separated by `; `. That would be a fault in
/// this crate's files; its tests check that it does not happen.
pub fn std_templates() -> Result<Vec<Template>, String> {
    let identity = ExtensionIdentity { identity: String::from(STDMAIN_PACKAGE), stem: String::from(STDMAIN_PACKAGE) };
    let files = StaticFiles::new(FILES.iter().copied(), STDMAIN_PACKAGE, STDMAIN_PACKAGE);
    flm_ext::templates::read_folder(&files, STDMAIN_PACKAGE, &identity).map_err(|errors| {
        errors.iter().map(|error| error.detail()).collect::<Vec<_>>().join("; ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flm_core::stdcore::features::compose::DEFAULT_TEMPLATE;
    use flm_ext::templates::TEMPLATE_FILE;

    /// The output and the name of the compose template that the description at `path`
    /// describes, by the folder it stands in.
    fn described(path: &str) -> Option<(&str, &str)> {
        let rest = path.strip_prefix("templates/")?;
        match rest.split('/').collect::<Vec<_>>().as_slice() {
            [output, file] => Some((*output, file.strip_suffix(".yaml")?)),
            [output, name, file] if *file == TEMPLATE_FILE => Some((*output, *name)),
            _ => None,
        }
    }

    /// The entries of `folder` with their names, leaving out the names that begin with `.`,
    /// as the build script does.
    fn entries(folder: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
        std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap())
            .map(|entry| (entry.file_name().into_string().unwrap(), entry.path()))
            .filter(|(name, _)| !name.starts_with('.'))
            .collect()
    }

    #[test]
    fn the_table_holds_every_description_of_the_folder() {
        let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/src/stdpkg/compose_standard/templates");
        let mut descriptions = Vec::new();
        for (output, output_path) in entries(std::path::Path::new(folder)) {
            for (name, entry_path) in entries(&output_path) {
                let path = format!("templates/{output}/{name}");
                if entry_path.join(TEMPLATE_FILE).is_file() {
                    descriptions.push(format!("{path}/{TEMPLATE_FILE}"));
                } else if name.ends_with(".yaml") {
                    descriptions.push(path);
                }
            }
        }
        assert!(!descriptions.is_empty());
        for description in &descriptions {
            assert!(FILES.iter().any(|(path, _)| path == description), "{description}");
        }
    }

    #[test]
    fn every_description_reads_and_serves_the_output_of_its_folder() {
        let templates = std_templates().unwrap();
        let descriptions: Vec<(&str, &str)> = FILES.iter().filter_map(|(path, _)| described(path)).collect();
        assert_eq!(templates.len(), descriptions.len());
        for ((output, name), template) in descriptions.iter().zip(&templates) {
            assert_eq!((template.output.as_str(), template.name.as_str()), (*output, *name));
            assert_eq!(template.package, STDMAIN_PACKAGE);
        }
    }

    #[test]
    fn every_output_with_templates_has_the_default_template() {
        let templates = std_templates().unwrap();
        for template in &templates {
            let output = &template.output;
            assert!(templates.iter().any(|other| other.output == *output && other.name == DEFAULT_TEMPLATE), "{output}");
        }
    }

    #[test]
    fn no_two_templates_share_an_output_and_a_name() {
        let templates = std_templates().unwrap();
        for (i, template) in templates.iter().enumerate() {
            let (output, name) = (&template.output, &template.name);
            assert!(!templates[..i].iter().any(|other| (&other.output, &other.name) == (output, name)), "{output} {name}");
        }
    }
}
