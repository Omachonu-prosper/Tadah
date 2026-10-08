//! Guards the Dependency Rule: inner layers must not name outer ones.

use std::fs;
use std::path::Path;

fn sources(dir: &str) -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((path.display().to_string(), fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join(dir), &mut out);
    out
}

fn assert_absent(dir: &str, forbidden: &[&str]) {
    for (file, text) in sources(dir) {
        for word in forbidden {
            assert!(!text.contains(word), "{file} must not mention `{word}`");
        }
    }
}

#[test]
fn domain_depends_on_nothing_else() {
    assert_absent(
        "src/domain",
        &["crate::application", "crate::infrastructure", "rusqlite", "eframe", "egui", "uuid", "chrono"],
    );
}

#[test]
fn application_does_not_know_adapters_or_frameworks() {
    assert_absent(
        "src/application",
        &["crate::infrastructure", "rusqlite", "eframe", "egui", "uuid", "std::time", "chrono"],
    );
}
