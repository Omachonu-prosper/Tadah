//! Headless command-line front end. Same use cases as the GUI, no terminal UI.
//!
//! `run` is generic over the store and takes its streams as arguments, so it is testable
//! in-process with the in-memory adapter.

use std::collections::HashMap;
use std::io::{self, Read, Write};

use crate::application::ports::Store;
use crate::application::{AppError, Board, CompletedKind, UseCases, notes};
use crate::domain::Status;
use crate::timefmt::format_local;

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

struct Cmd {
    name: &'static str,
    aliases: &'static [&'static str],
    usage: &'static str,
    about: &'static str,
}

const COMMANDS: &[Cmd] = &[
    Cmd {
        name: "ls",
        aliases: &["list"],
        usage: "ls [-o|--open] [SECTION]",
        about: "Show sections, tasks and subtasks with their numbers.\n\
                    -o, --open   hide finished tasks\n\
                    SECTION      only show this section (e.g. 2)",
    },
    Cmd {
        name: "completed",
        aliases: &["log"],
        usage: "completed",
        about: "Show finished tasks and subtasks with when they were completed, newest first.\n\
                    Each line starts with the item's number, so `tadah-cli reopen <number>` marks it\n\
                    not done again. Times are local.",
    },
    Cmd {
        name: "add",
        aliases: &[],
        usage: "add <SECTION> <TEXT...>",
        about: "Add a task to a section.   e.g.  tadah-cli add 1 review the PR",
    },
    Cmd {
        name: "add-section",
        aliases: &["section"],
        usage: "add-section <TITLE...>",
        about: "Add a new section.",
    },
    Cmd {
        name: "sub",
        aliases: &[],
        usage: "sub <TASK> <TEXT...>",
        about: "Add a subtask. Only partly done tasks accept new subtasks (see `start`).\n\
                    e.g.  tadah-cli sub 1.2 read chapter 3",
    },
    Cmd {
        name: "start",
        aliases: &["partial"],
        usage: "start <TASK...>",
        about: "Mark tasks partly done (*).",
    },
    Cmd {
        name: "done",
        aliases: &[],
        usage: "done <TASK|SUBTASK...>",
        about: "Mark tasks done (/) or subtasks checked.   e.g.  tadah-cli done 1.2 2.1.3",
    },
    Cmd {
        name: "reopen",
        aliases: &["undo"],
        usage: "reopen <TASK|SUBTASK...>",
        about: "Mark tasks not started (-) or subtasks unchecked.",
    },
    Cmd {
        name: "cycle",
        aliases: &[],
        usage: "cycle <TASK|SUBTASK...>",
        about: "Advance a task - -> * -> / -> -, or toggle a subtask (what clicking does in the app).",
    },
    Cmd {
        name: "edit",
        aliases: &["rename"],
        usage: "edit <SECTION|TASK|SUBTASK> <TEXT...>",
        about: "Change the text of a section title, task or subtask.",
    },
    Cmd {
        name: "rm",
        aliases: &["delete"],
        usage: "rm [-y|--yes] <SECTION|TASK|SUBTASK...>",
        about: "Delete sections, tasks or subtasks.\n\
                    Deleting a task removes its subtasks. Deleting a section removes everything in it\n\
                    and needs --yes when it isn't empty.",
    },
    Cmd {
        name: "import",
        aliases: &[],
        usage: "import [FILE|-]",
        about: "Append lists from notes-format text (FILE, or stdin when omitted or `-`).\n\
                    Lines: `- task`, `- * partly done`, `- / done`; indented `  - step` / `  - / step`\n\
                    lines are subtasks (unchecked / checked); any other line starts a new section.\n\
                    All-or-nothing.",
    },
    Cmd {
        name: "export",
        aliases: &[],
        usage: "export",
        about: "Print everything in notes format to stdout (round-trips with `import`).",
    },
];

const REFERENCES: &str = "\
REFERENCES:
    Items are addressed by the numbers `ls` prints:
        2        section 2
        2.3      task 3 in section 2
        2.3.1    subtask 1 of that task
    Numbers follow the current order, so they shift after deleting something; run `ls` again.";

fn global_help() -> String {
    let mut s = String::from(
        "tadah-cli - Tadah without the window (same data as the desktop app)\n\n\
         USAGE:\n    tadah-cli [COMMAND] [ARGS]\n    tadah-cli -h | --help | -V | --version\n\n\
         With no command, `ls` is run.\n\nCOMMANDS:\n",
    );
    let width = COMMANDS.iter().map(|c| c.usage.len()).max().unwrap_or(0);
    for c in COMMANDS {
        s.push_str(&format!("    {:<width$}  {}\n", c.usage, c.about.lines().next().unwrap_or("")));
    }
    s.push_str(&format!(
        "\n{REFERENCES}\n\nENVIRONMENT:\n    TADAH_DB    path to the database (default: ~/.local/share/tadah/tadah.db)\n\n\
         EXIT CODES:\n    0 ok    1 the operation failed    2 bad usage\n\n\
         Run `tadah-cli <COMMAND> -h` for details on one command.\n"
    ));
    s
}

fn command_help(c: &Cmd) -> String {
    let aliases = if c.aliases.is_empty() {
        String::new()
    } else {
        format!("\nALIASES: {}", c.aliases.join(", "))
    };
    format!("USAGE:\n    tadah-cli {}\n\n{}{aliases}\n\n{REFERENCES}\n", c.usage, c.about)
}

enum CliError {
    Usage(String),
    Failed(String),
    /// Reader of our stdout went away (`| head`); not an error.
    Quiet,
}

impl From<AppError> for CliError {
    fn from(e: AppError) -> Self {
        CliError::Failed(e.to_string())
    }
}

impl From<io::Error> for CliError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::BrokenPipe {
            CliError::Quiet
        } else {
            CliError::Failed(e.to_string())
        }
    }
}

type Res<T = ()> = Result<T, CliError>;

fn usage<T>(msg: impl Into<String>) -> Res<T> {
    Err(CliError::Usage(msg.into()))
}

/// Runs one command line. Returns the process exit code.
pub fn run<S: Store>(
    uc: &UseCases<S>,
    args: &[String],
    stdin: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match dispatch(uc, args, stdin, out) {
        Ok(()) | Err(CliError::Quiet) => EXIT_OK,
        Err(CliError::Usage(m)) => {
            let _ = writeln!(err, "error: {m}\nTry `tadah-cli --help`.");
            EXIT_USAGE
        }
        Err(CliError::Failed(m)) => {
            let _ = writeln!(err, "error: {m}");
            EXIT_FAILED
        }
    }
}

fn is_help(a: &str) -> bool {
    matches!(a, "-h" | "--help")
}

/// True when the command line only asks for help or the version, so no database is needed.
pub fn is_help_request(args: &[String]) -> bool {
    match args {
        [a, ..] if is_help(a) || matches!(a.as_str(), "help" | "-V" | "--version") => true,
        [_, b, ..] => is_help(b),
        _ => false,
    }
}

fn dispatch<S: Store>(
    uc: &UseCases<S>,
    args: &[String],
    stdin: &mut dyn Read,
    out: &mut dyn Write,
) -> Res {
    let Some(first) = args.first().map(String::as_str) else {
        return ls(uc, &[], out);
    };
    if is_help(first) || first == "help" {
        // `help <command>` works too.
        if let Some(c) = args.get(1).and_then(|n| find(n)) {
            write!(out, "{}", command_help(c))?;
        } else {
            write!(out, "{}", global_help())?;
        }
        return Ok(());
    }
    if matches!(first, "-V" | "--version") {
        writeln!(out, "tadah-cli {}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }

    let Some(cmd) = find(first) else {
        return usage(format!("unknown command `{first}`"));
    };
    let rest = &args[1..];
    if rest.first().is_some_and(|a| is_help(a)) {
        write!(out, "{}", command_help(cmd))?;
        return Ok(());
    }

    match cmd.name {
        "ls" => ls(uc, rest, out),
        "completed" => completed(uc, rest, out),
        "add" => add(uc, rest, out),
        "add-section" => add_section(uc, rest, out),
        "sub" => sub(uc, rest, out),
        "start" => set_status(uc, rest, Change::Start, out),
        "done" => set_status(uc, rest, Change::Done, out),
        "reopen" => set_status(uc, rest, Change::Reopen, out),
        "cycle" => set_status(uc, rest, Change::Cycle, out),
        "edit" => edit(uc, rest, out),
        "rm" => rm(uc, rest, out),
        "import" => import(uc, rest, stdin, out),
        "export" => {
            writeln!(out, "{}", notes::serialize(&uc.board()?))?;
            Ok(())
        }
        other => unreachable!("command table and dispatch out of sync: {other}"),
    }
}

fn find(name: &str) -> Option<&'static Cmd> {
    COMMANDS.iter().find(|c| c.name == name || c.aliases.contains(&name))
}

// ---- references ---------------------------------------------------------

/// What a reference points at, with everything the handlers need (owned, so no borrows
/// of the board outlive resolution).
enum Target {
    Section { id: String, tasks: usize },
    Task { id: String, text: String },
    Subtask { id: String, text: String },
}

fn parse_ref(s: &str) -> Res<Vec<usize>> {
    let parts: Option<Vec<usize>> =
        s.split('.').map(|p| p.parse::<usize>().ok().filter(|n| *n >= 1)).collect();
    match parts {
        Some(p) if (1..=3).contains(&p.len()) => Ok(p),
        _ => usage(format!("invalid reference `{s}` (expected N, N.N or N.N.N)")),
    }
}

fn resolve(board: &Board, reference: &str) -> Res<Target> {
    let path = parse_ref(reference)?;
    let missing = |what: &str| CliError::Failed(format!("no {what} {reference}"));

    let list = board.lists.get(path[0] - 1).ok_or_else(|| missing("section"))?;
    if path.len() == 1 {
        return Ok(Target::Section { id: list.id.clone(), tasks: list.tasks.len() });
    }
    let task = list.tasks.get(path[1] - 1).ok_or_else(|| missing("task"))?;
    if path.len() == 2 {
        return Ok(Target::Task { id: task.id.clone(), text: task.text.clone() });
    }
    let sub = task.subtasks.get(path[2] - 1).ok_or_else(|| missing("subtask"))?;
    Ok(Target::Subtask { id: sub.id.clone(), text: sub.text.clone() })
}

fn join_text(words: &[String], what: &str) -> Res<String> {
    let text = words.join(" ");
    if text.trim().is_empty() {
        return usage(format!("missing {what}"));
    }
    Ok(text)
}

// ---- commands -----------------------------------------------------------

fn status_marker(s: Status) -> &'static str {
    match s {
        Status::Todo => "-",
        Status::Partial => "*",
        Status::Done => "/",
    }
}

fn ls<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    let mut open_only = false;
    let mut only: Option<usize> = None;
    for a in rest {
        match a.as_str() {
            "-o" | "--open" => open_only = true,
            f if f.starts_with('-') => return usage(format!("unknown option `{f}` for ls")),
            r => {
                let path = parse_ref(r)?;
                if path.len() != 1 {
                    return usage("ls takes a section number, e.g. `ls 2`");
                }
                only = Some(path[0]);
            }
        }
    }

    let board = uc.board()?;
    if let Some(n) = only
        && n > board.lists.len()
    {
        return Err(CliError::Failed(format!("no section {n}")));
    }
    if board.lists.is_empty() {
        writeln!(out, "(nothing yet: `tadah-cli add-section <title>` to start a list)")?;
        return Ok(());
    }

    for (i, list) in board.lists.iter().enumerate() {
        let n = i + 1;
        if only.is_some_and(|o| o != n) {
            continue;
        }
        let title = if list.title.is_empty() { "(untitled)" } else { list.title.as_str() };
        writeln!(out, "{n}  {title}")?;
        for (j, task) in list.tasks.iter().enumerate() {
            if open_only && task.status == Status::Done {
                continue;
            }
            let progress = if task.subtasks.is_empty() {
                String::new()
            } else {
                let done = task.subtasks.iter().filter(|s| s.done).count();
                format!("  ({done}/{})", task.subtasks.len())
            };
            writeln!(out, "  {n}.{}  {}  {}{progress}", j + 1, status_marker(task.status), task.text)?;
            for (k, sub) in task.subtasks.iter().enumerate() {
                writeln!(
                    out,
                    "       {n}.{}.{}  [{}]  {}",
                    j + 1,
                    k + 1,
                    if sub.done { "x" } else { " " },
                    sub.text
                )?;
            }
        }
    }
    writeln!(
        out,
        "\n{} open · {} partial · {} done",
        board.count(Status::Todo),
        board.count(Status::Partial),
        board.count(Status::Done)
    )?;
    Ok(())
}

fn completed<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    if let Some(extra) = rest.first() {
        return usage(format!("completed takes no arguments (got `{extra}`)"));
    }
    let board = uc.board()?;
    let items = uc.completed()?;
    if items.is_empty() {
        writeln!(out, "(nothing completed yet)")?;
        return Ok(());
    }

    // The same numbers `ls` prints, so results can be fed to `reopen`.
    let mut numbers: HashMap<&str, String> = HashMap::new();
    for (i, list) in board.lists.iter().enumerate() {
        for (j, task) in list.tasks.iter().enumerate() {
            numbers.insert(&task.id, format!("{}.{}", i + 1, j + 1));
            for (k, sub) in task.subtasks.iter().enumerate() {
                numbers.insert(&sub.id, format!("{}.{}.{}", i + 1, j + 1, k + 1));
            }
        }
    }

    for item in &items {
        let number = numbers.get(item.id.as_str()).map_or("?", String::as_str);
        let (marker, context) = match (&item.kind, &item.parent_task) {
            (CompletedKind::Subtask, Some(parent)) => ("[x]", format!("  (under: {parent})")),
            _ if item.section.is_empty() => ("/  ", String::new()),
            _ => ("/  ", format!("  [{}]", item.section)),
        };
        writeln!(
            out,
            "{}  {number:<6} {marker}  {}{context}",
            format_local(item.completed_at),
            item.text
        )?;
    }
    Ok(())
}

fn add<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    let [section, words @ ..] = rest else {
        return usage("usage: add <SECTION> <TEXT...>");
    };
    let text = join_text(words, "task text")?;
    let board = uc.board()?;
    let Target::Section { id, tasks, .. } = resolve(&board, section)? else {
        return usage("`add` needs a section number (like `2`); to add a subtask use `sub`");
    };
    uc.add_task(&id, &text)?;
    writeln!(out, "Added {section}.{}", tasks + 1)?;
    Ok(())
}

fn add_section<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    let title = join_text(rest, "section title")?;
    let count = uc.board()?.lists.len();
    uc.create_list(&title)?;
    writeln!(out, "Added section {}", count + 1)?;
    Ok(())
}

fn sub<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    let [task, words @ ..] = rest else {
        return usage("usage: sub <TASK> <TEXT...>");
    };
    let text = join_text(words, "subtask text")?;
    let board = uc.board()?;
    let Target::Task { id, .. } = resolve(&board, task)? else {
        return usage("`sub` needs a task number (like `1.2`)");
    };
    uc.add_subtask(&id, &text)?;
    // The new subtask is appended, so its number is one past the current count.
    let path = parse_ref(task)?;
    let existing = board.lists[path[0] - 1].tasks[path[1] - 1].subtasks.len();
    writeln!(out, "Added {task}.{}", existing + 1)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum Change {
    Start,
    Done,
    Reopen,
    Cycle,
}

fn set_status<S: Store>(
    uc: &UseCases<S>,
    refs: &[String],
    change: Change,
    out: &mut dyn Write,
) -> Res {
    if refs.is_empty() {
        return usage("give at least one task or subtask number");
    }
    // Resolve everything first so a bad reference changes nothing.
    let board = uc.board()?;
    let targets: Vec<(&String, Target)> =
        refs.iter().map(|r| resolve(&board, r).map(|t| (r, t))).collect::<Res<_>>()?;

    for (reference, target) in &targets {
        match (target, change) {
            (Target::Section { .. }, _) => {
                return usage(format!("{reference} is a section; give a task or subtask number"));
            }
            (Target::Subtask { .. }, Change::Start) => {
                return usage(format!("{reference} is a subtask; subtasks are only checked or unchecked"));
            }
            _ => {}
        }
    }

    for (reference, target) in targets {
        match target {
            Target::Task { id, text } => {
                let status = match change {
                    Change::Start => Status::Partial,
                    Change::Done => Status::Done,
                    Change::Reopen => Status::Todo,
                    Change::Cycle => uc.advance_task(&id)?,
                };
                if !matches!(change, Change::Cycle) {
                    uc.set_task_status(&id, status)?;
                }
                writeln!(out, "{reference}  {}  {text}", status_marker(status))?;
            }
            Target::Subtask { id, text } => {
                let done = match change {
                    Change::Done => {
                        uc.set_subtask_done(&id, true)?;
                        true
                    }
                    Change::Reopen => {
                        uc.set_subtask_done(&id, false)?;
                        false
                    }
                    _ => uc.toggle_subtask(&id)?,
                };
                writeln!(out, "{reference}  [{}]  {text}", if done { "x" } else { " " })?;
            }
            Target::Section { .. } => unreachable!("rejected above"),
        }
    }
    Ok(())
}

fn edit<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    let [reference, words @ ..] = rest else {
        return usage("usage: edit <SECTION|TASK|SUBTASK> <TEXT...>");
    };
    let text = join_text(words, "new text")?;
    match resolve(&uc.board()?, reference)? {
        Target::Section { id, .. } => uc.rename_list(&id, &text)?,
        Target::Task { id, .. } => uc.edit_task(&id, &text)?,
        Target::Subtask { id, .. } => uc.edit_subtask(&id, &text)?,
    }
    writeln!(out, "Updated {reference}")?;
    Ok(())
}

fn rm<S: Store>(uc: &UseCases<S>, rest: &[String], out: &mut dyn Write) -> Res {
    let yes = rest.iter().any(|a| a == "-y" || a == "--yes");
    let refs: Vec<&String> = rest.iter().filter(|a| *a != "-y" && *a != "--yes").collect();
    if refs.is_empty() {
        return usage("give at least one section, task or subtask number");
    }

    // Resolve all first (by id), because numbers shift as things are deleted.
    let board = uc.board()?;
    let targets: Vec<(&String, Target)> =
        refs.iter().map(|r| resolve(&board, r).map(|t| (*r, t))).collect::<Res<_>>()?;
    for (reference, target) in &targets {
        if let Target::Section { tasks, .. } = target
            && *tasks > 0
            && !yes
        {
            return Err(CliError::Failed(format!(
                "section {reference} has {tasks} task(s); pass --yes to delete it and everything in it"
            )));
        }
    }

    for (reference, target) in targets {
        match target {
            Target::Section { id, .. } => uc.delete_list(&id)?,
            Target::Task { id, .. } => uc.delete_task(&id)?,
            Target::Subtask { id, .. } => uc.delete_subtask(&id)?,
        }
        writeln!(out, "Deleted {reference}")?;
    }
    Ok(())
}

fn import<S: Store>(
    uc: &UseCases<S>,
    rest: &[String],
    stdin: &mut dyn Read,
    out: &mut dyn Write,
) -> Res {
    let mut text = String::new();
    match rest {
        [] => {
            stdin.read_to_string(&mut text).map_err(|e| CliError::Failed(format!("reading stdin: {e}")))?;
        }
        [p] if p == "-" => {
            stdin.read_to_string(&mut text).map_err(|e| CliError::Failed(format!("reading stdin: {e}")))?;
        }
        [path] if !path.starts_with('-') => {
            text = std::fs::read_to_string(path)
                .map_err(|e| CliError::Failed(format!("could not read {path}: {e}")))?;
        }
        _ => return usage("usage: import [FILE|-]"),
    }
    let (lists, tasks) = uc.import_notes(&text)?;
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    writeln!(out, "Imported {} in {}", plural(tasks, "task", "tasks"), plural(lists, "section", "sections"))?;
    Ok(())
}
