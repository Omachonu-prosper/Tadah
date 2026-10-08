//! Composition root: the only place that picks concrete adapters.

mod ui;

use tadah::composition::{db_path, open_app};

/// Re-launches this binary detached from the terminal so the shell gets its prompt back.
/// The child runs with `--foreground`. Returns false if the spawn failed (caller then
/// just runs in the foreground).
#[cfg(unix)]
fn spawn_detached() -> bool {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let Ok(exe) = std::env::current_exe() else { return false };
    Command::new(exe)
        .arg("--foreground")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // Own process group: closing the terminal won't take the app down with it.
        .process_group(0)
        .spawn()
        .is_ok()
}

#[cfg(not(unix))]
fn spawn_detached() -> bool {
    false
}

fn main() -> eframe::Result {
    let mut args = std::env::args().skip(1).peekable();
    let foreground = args.peek().is_some_and(|a| a == "--foreground");
    if foreground {
        args.next();
    } else if args.peek().is_none() && spawn_detached() {
        return Ok(());
    }

    let path = db_path();
    let uc = open_app(&path).unwrap_or_else(|e| {
        eprintln!("Could not open database at {}: {e}", path.display());
        std::process::exit(1);
    });

    // `tadah --import notes.txt` appends a notes-format file, then exits.
    if let Some(flag) = args.next() {
        if flag == "--import" {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("usage: tadah --import <file>");
                std::process::exit(2);
            });
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("Could not read {path}: {e}");
                std::process::exit(1);
            });
            match uc.import_notes(&text) {
                Ok((lists, tasks)) => println!("Imported {tasks} tasks in {lists} lists"),
                Err(e) => {
                    eprintln!("Import failed: {e}");
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        eprintln!("unknown argument {flag}; usage: tadah [--foreground | --import <file>]");
        std::process::exit(2);
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Tadah")
            .with_inner_size([760.0, 720.0])
            .with_min_inner_size([420.0, 320.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tadah",
        options,
        Box::new(|_cc| Ok(Box::new(ui::TadahApp::new(uc)))),
    )
}
