//! An editor for the tests of opening a place in the editor: a script of the test's own,
//! found as an editor is, which writes the arguments it is given into a file, so that no
//! editor is opened. For systems with a shell (`#!/bin/sh`).

#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

/// The upper bound of every wait.
const BOUND: Duration = Duration::from_secs(30);

/// A folder of its own for the test `name`, empty, under the build's folder for tests.
pub fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("editor").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Writes, in `folder`, a program named `name` that writes the arguments it is given, one
/// per line, into the file `record` (written whole, under another name first). `name` may
/// be a path relative to `folder`, such as the place of a program inside an application
/// bundle; the folders on that path are made.
pub fn fake_editor(folder: &Path, name: &str, record: &Path) {
    let program = folder.join(name);
    std::fs::create_dir_all(program.parent().unwrap()).unwrap();
    std::fs::write(&program, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{0}.part'\nmv '{0}.part' '{0}'\n", record.display()))
        .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Writes, in `folder`, a program named `name` that writes `said` on its standard error and
/// ends with the exit status 3.
pub fn failing_editor(folder: &Path, name: &str, said: &str) {
    let program = folder.join(name);
    std::fs::write(&program, format!("#!/bin/sh\nprintf '%s\\n' '{said}' >&2\nexit 3\n")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The lines of `record` once the program has written it.
pub fn recorded(record: &Path) -> Vec<String> {
    let started = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(record) {
            return text.lines().map(str::to_owned).collect();
        }
        assert!(started.elapsed() < BOUND, "the editor was not run within the bound");
        thread::sleep(Duration::from_millis(20));
    }
}
