//! Headless Tadah. Thin shell: wire adapters, hand the arguments to `tadah::cli::run`.

use std::io;

use tadah::application::UseCases;
use tadah::cli;
use tadah::composition::{db_path, open_app};
use tadah::infrastructure::memory::MemoryStore;
use tadah::infrastructure::system::{SystemClock, UuidGenerator};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let (stdin, stdout, stderr) = (io::stdin(), io::stdout(), io::stderr());
    let (mut stdin, mut stdout, mut stderr) = (stdin.lock(), stdout.lock(), stderr.lock());

    let code = if cli::is_help_request(&args) {
        // Help and version never read data, so don't open (or create) the database.
        let app = UseCases::new(
            MemoryStore::default(),
            Box::new(SystemClock),
            Box::new(UuidGenerator),
        );
        cli::run(&app, &args, &mut stdin, &mut stdout, &mut stderr)
    } else {
        let path = db_path();
        match open_app(&path) {
            Ok(app) => cli::run(&app, &args, &mut stdin, &mut stdout, &mut stderr),
            Err(e) => {
                eprintln!("error: could not open database at {}: {e}", path.display());
                cli::EXIT_FAILED
            }
        }
    };
    std::process::exit(code);
}
