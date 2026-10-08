use std::cell::Cell;

use tadah::application::UseCases;
use tadah::application::ports::{Clock, IdGenerator};
use tadah::cli;
use tadah::infrastructure::memory::MemoryStore;

struct TickClock(Cell<i64>);
impl Clock for TickClock {
    fn now(&self) -> i64 {
        self.0.set(self.0.get() + 1);
        self.0.get()
    }
}

struct SeqIds(Cell<u32>);
impl IdGenerator for SeqIds {
    fn next(&self) -> String {
        self.0.set(self.0.get() + 1);
        format!("id-{}", self.0.get())
    }
}

struct Cli(UseCases<MemoryStore>);

struct Out {
    code: i32,
    out: String,
    err: String,
}

impl Cli {
    fn new() -> Self {
        Cli(UseCases::new(
            MemoryStore::default(),
            Box::new(TickClock(Cell::new(0))),
            Box::new(SeqIds(Cell::new(0))),
        ))
    }

    fn run_with_stdin(&self, args: &[&str], stdin: &str) -> Out {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = cli::run(&self.0, &args, &mut stdin.as_bytes(), &mut out, &mut err);
        Out {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    fn run(&self, args: &[&str]) -> Out {
        self.run_with_stdin(args, "")
    }

    /// Runs a command that must succeed.
    fn ok(&self, args: &[&str]) -> String {
        let r = self.run(args);
        assert_eq!(r.code, 0, "{args:?} failed: {}", r.err);
        r.out
    }
}

#[test]
fn help_goes_to_stdout_with_exit_zero() {
    let cli = Cli::new();
    for flag in ["-h", "--help", "help"] {
        let r = cli.run(&[flag]);
        assert_eq!(r.code, 0);
        assert!(r.out.contains("USAGE:") && r.out.contains("COMMANDS:") && r.out.contains("REFERENCES:"));
        assert!(r.err.is_empty());
    }
    let r = cli.run(&["--version"]);
    assert!(r.out.starts_with("tadah-cli "));
}

#[test]
fn every_command_has_help() {
    let cli = Cli::new();
    let global = cli.run(&["-h"]).out;
    for cmd in [
        "ls", "completed", "add", "add-section", "sub", "start", "done", "reopen", "cycle", "edit", "rm", "import",
        "export",
    ] {
        assert!(global.contains(cmd), "{cmd} missing from global help");
        let r = cli.run(&[cmd, "-h"]);
        assert_eq!(r.code, 0, "{cmd} -h");
        assert!(r.out.contains(&format!("tadah-cli {cmd}")), "{cmd} usage line: {}", r.out);
    }
}

#[test]
fn unknown_command_and_bad_reference_are_usage_errors() {
    let cli = Cli::new();
    let r = cli.run(&["frobnicate"]);
    assert_eq!(r.code, 2);
    assert!(r.err.contains("unknown command") && r.out.is_empty());

    assert_eq!(cli.run(&["done", "x.y"]).code, 2);
    assert_eq!(cli.run(&["done", "1.2.3.4"]).code, 2);
    assert_eq!(cli.run(&["done"]).code, 2);
    assert_eq!(cli.run(&["add", "1"]).code, 2);
}

#[test]
fn missing_items_are_runtime_failures() {
    let cli = Cli::new();
    let r = cli.run(&["done", "1.1"]);
    assert_eq!((r.code, r.err.as_str()), (1, "error: no section 1.1\n"));
}

#[test]
fn empty_board_hint_and_default_command_is_ls() {
    let cli = Cli::new();
    let r = cli.run(&[]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("add-section"));
}

#[test]
fn add_list_and_track_progress() {
    let cli = Cli::new();
    assert_eq!(cli.ok(&["add-section", "Afara.AI", "Todos"]), "Added section 1\n");
    assert_eq!(cli.ok(&["add", "1", "Session", "thread", "storage"]), "Added 1.1\n");
    assert_eq!(cli.ok(&["add", "1", "PDF generation"]), "Added 1.2\n");
    assert_eq!(cli.ok(&["start", "1.1"]), "1.1  *  Session thread storage\n");
    assert_eq!(cli.ok(&["done", "1.2"]), "1.2  /  PDF generation\n");

    let ls = cli.ok(&["ls"]);
    assert!(ls.contains("1  Afara.AI Todos"));
    assert!(ls.contains("  1.1  *  Session thread storage"));
    assert!(ls.contains("  1.2  /  PDF generation"));
    assert!(ls.contains("0 open · 1 partial · 1 done"));
}

#[test]
fn subtasks_need_a_partly_done_task() {
    let cli = Cli::new();
    cli.ok(&["add-section", "Work"]);
    cli.ok(&["add", "1", "book"]);

    let r = cli.run(&["sub", "1.1", "ch 1"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("partly done"));

    cli.ok(&["start", "1.1"]);
    assert_eq!(cli.ok(&["sub", "1.1", "ch", "1"]), "Added 1.1.1\n");
    assert_eq!(cli.ok(&["sub", "1.1", "ch 2"]), "Added 1.1.2\n");
    assert_eq!(cli.ok(&["done", "1.1.1"]), "1.1.1  [x]  ch 1\n");

    let ls = cli.ok(&["ls"]);
    assert!(ls.contains("1.1  *  book  (1/2)"));
    assert!(ls.contains("1.1.1  [x]  ch 1"));
    assert!(ls.contains("1.1.2  [ ]  ch 2"));

    assert_eq!(cli.ok(&["reopen", "1.1.1"]), "1.1.1  [ ]  ch 1\n");
    assert_eq!(cli.ok(&["cycle", "1.1.2"]), "1.1.2  [x]  ch 2\n");
    assert_eq!(cli.run(&["start", "1.1.1"]).code, 2);
}

#[test]
fn cycle_walks_through_the_three_states() {
    let cli = Cli::new();
    cli.ok(&["add-section", "S"]);
    cli.ok(&["add", "1", "t"]);
    assert_eq!(cli.ok(&["cycle", "1.1"]), "1.1  *  t\n");
    assert_eq!(cli.ok(&["cycle", "1.1"]), "1.1  /  t\n");
    assert_eq!(cli.ok(&["cycle", "1.1"]), "1.1  -  t\n");
}

#[test]
fn several_refs_are_validated_before_any_change() {
    let cli = Cli::new();
    cli.ok(&["add-section", "S"]);
    cli.ok(&["add", "1", "a"]);
    cli.ok(&["add", "1", "b"]);

    let r = cli.run(&["done", "1.1", "1.9"]);
    assert_eq!(r.code, 1);
    // Nothing changed, even though 1.1 was valid.
    assert!(cli.ok(&["ls"]).contains("2 open · 0 partial · 0 done"));

    assert_eq!(cli.ok(&["done", "1.1", "1.2"]).lines().count(), 2);
}

#[test]
fn ls_open_hides_finished_tasks_but_keeps_numbers() {
    let cli = Cli::new();
    cli.ok(&["add-section", "S"]);
    cli.ok(&["add", "1", "first"]);
    cli.ok(&["add", "1", "second"]);
    cli.ok(&["done", "1.1"]);

    let ls = cli.ok(&["ls", "--open"]);
    assert!(!ls.contains("first"));
    assert!(ls.contains("1.2  -  second"));
}

#[test]
fn ls_can_target_one_section() {
    let cli = Cli::new();
    cli.ok(&["add-section", "A"]);
    cli.ok(&["add-section", "B"]);
    cli.ok(&["add", "2", "in b"]);
    let ls = cli.ok(&["ls", "2"]);
    assert!(ls.contains("2  B") && !ls.contains("1  A"));
    assert_eq!(cli.run(&["ls", "9"]).code, 1);
    assert_eq!(cli.run(&["ls", "1.1"]).code, 2);
}

#[test]
fn edit_works_at_every_level() {
    let cli = Cli::new();
    cli.ok(&["add-section", "S"]);
    cli.ok(&["add", "1", "t"]);
    cli.ok(&["start", "1.1"]);
    cli.ok(&["sub", "1.1", "s"]);
    cli.ok(&["edit", "1", "New", "title"]);
    cli.ok(&["edit", "1.1", "new task"]);
    cli.ok(&["edit", "1.1.1", "new sub"]);
    let ls = cli.ok(&["ls"]);
    assert!(ls.contains("1  New title") && ls.contains("new task") && ls.contains("new sub"));
    assert_eq!(cli.run(&["edit", "1.1", "   "]).code, 2);
}

#[test]
fn rm_asks_for_yes_on_non_empty_sections_and_shifting_numbers_are_safe() {
    let cli = Cli::new();
    cli.ok(&["add-section", "S"]);
    for t in ["a", "b", "c"] {
        cli.ok(&["add", "1", t]);
    }

    let r = cli.run(&["rm", "1"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("--yes"));
    assert!(cli.ok(&["ls"]).contains("3 open"));

    // Deleting 1.1 and 1.3 together removes a and c, not whatever shifts into place.
    cli.ok(&["rm", "1.1", "1.3"]);
    let ls = cli.ok(&["ls"]);
    assert!(ls.contains("  b") && !ls.contains("  a\n") && !ls.contains("  c\n"));

    assert_eq!(cli.ok(&["rm", "-y", "1"]), "Deleted 1\n");
    assert!(cli.ok(&["ls"]).contains("nothing yet"));
}

#[test]
fn import_from_stdin_and_export_round_trip() {
    let cli = Cli::new();
    // Canonical form (the exporter always writes the marker first).
    let notes = "- AfaraAI review\n- * SWE book\n  - / ch 1\n  - ch 2\n\nAfara.AI Todos\n- / Text replace\n- Plain";
    let r = cli.run_with_stdin(&["import"], notes);
    assert_eq!((r.code, r.out.as_str()), (0, "Imported 4 tasks in 2 sections\n"));

    let exported = cli.ok(&["export"]);
    assert_eq!(exported.trim_end(), notes);

    // Re-importing what we exported (into a fresh store) reproduces it.
    let other = Cli::new();
    other.run_with_stdin(&["import", "-"], &exported);
    assert_eq!(other.ok(&["export"]), exported);
}

#[test]
fn bad_import_changes_nothing() {
    let cli = Cli::new();
    let r = cli.run_with_stdin(&["import"], &format!("Good\n- fine\n- {}", "x".repeat(501)));
    assert_eq!(r.code, 1);
    assert!(cli.ok(&["ls"]).contains("nothing yet"));
}

#[test]
fn completed_lists_finished_items_newest_first_and_reopen_removes_them() {
    let cli = Cli::new();
    assert!(cli.ok(&["completed"]).contains("nothing completed yet"));

    cli.ok(&["add-section", "Work"]);
    cli.ok(&["add", "1", "first"]);
    cli.ok(&["add", "1", "book"]);
    cli.ok(&["start", "1.2"]);
    cli.ok(&["sub", "1.2", "chapter"]);

    cli.ok(&["done", "1.1"]);
    cli.ok(&["done", "1.2.1"]);

    let out = cli.ok(&["completed"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    // Newest first: the subtask was completed after the task.
    assert!(lines[0].contains("1.2.1") && lines[0].contains("[x]  chapter"), "{}", lines[0]);
    assert!(lines[0].contains("(under: book)"));
    assert!(lines[1].contains("1.1 ") && lines[1].contains("/    first") && lines[1].contains("[Work]"), "{}", lines[1]);
    // Timestamps come first, formatted as a date and time.
    assert!(lines[0].starts_with("1970-01-01 "), "{}", lines[0]);

    // The number shown is usable with reopen.
    cli.ok(&["reopen", "1.2.1"]);
    cli.ok(&["reopen", "1.1"]);
    assert!(cli.ok(&["completed"]).contains("nothing completed yet"));

    assert_eq!(cli.run(&["completed", "extra"]).code, 2);
    assert_eq!(cli.run(&["log"]).code, 0);
}

#[test]
fn help_requests_are_detected_without_needing_a_database() {
    let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(cli::is_help_request(&a(&["-h"])));
    assert!(cli::is_help_request(&a(&["add", "-h"])));
    assert!(cli::is_help_request(&a(&["--version"])));
    assert!(!cli::is_help_request(&a(&["ls"])));
    assert!(!cli::is_help_request(&a(&["add", "1", "-h"])));
    assert!(!cli::is_help_request(&a(&[])));
}
