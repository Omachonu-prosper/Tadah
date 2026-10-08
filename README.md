# Tadah

A small native todo app. No server: one binary, one SQLite file.

Each task is open `-`, partly done `*`, or done `/`, matching the plain-text notes convention. Click a task's box to cycle through the three states. Tasks are grouped into sections.

## Run

```
cargo run --release        # or run target/release/tadah after building
```

**Subtasks:** a partly done (`*`) task shows an "+ add subtask" field; each subtask is a checkbox and the task row shows progress like `2/3`. In notes, subtasks are indented lines under their task (`  - / step` = done).

**Completed tab:** every finished task and subtask, newest first, with the time it was completed (local time) and where it lives. *Reopen* marks it not done again (a task goes back to `-`, a subtask unchecked) and it returns to the Todo tab.

## Headless: `tadah-cli`

The same app without a window, for scripts and SSH. It shares the database with the GUI. `tadah-cli -h` lists everything; `tadah-cli <command> -h` explains one command.

```
tadah-cli                      # same as `ls`
tadah-cli ls --open            # hide finished tasks
tadah-cli add 2 review the PR  # add a task to section 2
tadah-cli start 2.3            # mark task 2.3 partly done (*)
tadah-cli sub 2.3 read ch 3    # add a subtask (only on partly done tasks)
tadah-cli done 2.3.1 2.4       # check a subtask, finish a task
tadah-cli completed            # finished items with timestamps; `reopen <number>` undoes one
tadah-cli export > notes.txt   # notes format; `import` reads it back (file or stdin)
```

Items are addressed by the numbers `ls` prints: `2` section, `2.3` task, `2.3.1` subtask. Exit codes: 0 ok, 1 the operation failed, 2 bad usage. Errors go to stderr.

Data lives in `~/.local/share/tadah/tadah.db` (override with `TADAH_DB=/path/to/file.db`).

Import a notes file from the command line:

```
tadah --import notes.txt
```

Lines starting with `- ` are tasks (`- * ` partly done, `- / ` done). Any other line starts a new section. The same import, plus "Copy all as text" export, is in the app under *import / export*.

## Layout

```
src/domain/          entities and rules (Task, TodoList, Status)   depends on nothing
src/application/     use cases, ports (traits), notes format        depends on domain
src/infrastructure/  SQLite adapter, in-memory adapter, clock/ids   implements the ports
src/ui.rs, main.rs   egui front end and composition root
```

`cargo test` runs the domain and use-case tests against both the in-memory and SQLite adapters, and `tests/dependency_rule.rs` fails if an inner layer starts importing an outer one.
