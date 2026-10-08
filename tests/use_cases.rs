use std::cell::Cell;

use tadah::application::ports::{Clock, IdGenerator, Store};
use tadah::application::{AppError, UseCases};
use tadah::domain::Status;
use tadah::infrastructure::memory::MemoryStore;
use tadah::infrastructure::sqlite::SqliteStore;

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

fn app<S: Store>(store: S) -> UseCases<S> {
    UseCases::new(store, Box::new(TickClock(Cell::new(0))), Box::new(SeqIds(Cell::new(0))))
}

// Each scenario is written once and run against every adapter.

fn add_edit_advance_delete<S: Store>(app: UseCases<S>) {
    let list = app.create_list("Work").unwrap();
    let task = app.add_task(&list, "  write tests ").unwrap();
    assert_eq!(task.text, "write tests");

    assert_eq!(app.advance_task(&task.id).unwrap(), Status::Partial);
    assert_eq!(app.advance_task(&task.id).unwrap(), Status::Done);
    assert_eq!(app.advance_task(&task.id).unwrap(), Status::Todo);

    app.edit_task(&task.id, "write more tests").unwrap();
    let board = app.board().unwrap();
    assert_eq!(board.lists[0].tasks[0].text, "write more tests");
    assert_eq!(board.count(Status::Todo), 1);

    app.delete_task(&task.id).unwrap();
    assert!(app.board().unwrap().lists[0].tasks.is_empty());
}

fn keeps_insertion_order<S: Store>(app: UseCases<S>) {
    let a = app.create_list("A").unwrap();
    let b = app.create_list("B").unwrap();
    for t in ["one", "two", "three"] {
        app.add_task(&a, t).unwrap();
    }
    app.add_task(&b, "other").unwrap();
    let board = app.board().unwrap();
    assert_eq!(
        board.lists.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(),
        ["A", "B"]
    );
    assert_eq!(
        board.lists[0].tasks.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
        ["one", "two", "three"]
    );
}

fn rejects_bad_input<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    assert!(matches!(app.add_task(&list, "   "), Err(AppError::Validation(_))));
    assert!(matches!(app.add_task("missing", "x"), Err(AppError::NotFound(_))));
    assert!(matches!(app.advance_task("missing"), Err(AppError::NotFound(_))));
    assert!(matches!(app.rename_list("missing", "x"), Err(AppError::NotFound(_))));
}

fn deleting_a_list_deletes_its_tasks<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    let task = app.add_task(&list, "x").unwrap();
    app.delete_list(&list).unwrap();
    assert!(app.board().unwrap().lists.is_empty());
    assert!(matches!(app.advance_task(&task.id), Err(AppError::NotFound(_))));
}

fn import_appends_and_keeps_statuses<S: Store>(app: UseCases<S>) {
    app.create_list("Existing").unwrap();
    let counts = app
        .import_notes("- a\n- SWE book *\n\nAfara.AI Todos\n- / done one\n- plain")
        .unwrap();
    assert_eq!(counts, (2, 4));

    let board = app.board().unwrap();
    assert_eq!(board.lists.len(), 3);
    assert_eq!(board.lists[0].title, "Existing");
    assert_eq!(board.lists[2].title, "Afara.AI Todos");
    assert_eq!(board.lists[1].tasks[1].status, Status::Partial);
    assert_eq!(board.lists[2].tasks[0].status, Status::Done);
}

fn import_is_all_or_nothing<S: Store>(app: UseCases<S>) {
    let too_long = "x".repeat(501);
    let err = app.import_notes(&format!("Good\n- fine\n- {too_long}")).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
    assert!(app.board().unwrap().lists.is_empty());
}

fn subtasks_only_on_partial_tasks<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    let task = app.add_task(&list, "book").unwrap();

    // Not started: rejected.
    assert!(matches!(app.add_subtask(&task.id, "ch 1"), Err(AppError::Validation(_))));

    app.advance_task(&task.id).unwrap(); // partial
    app.add_subtask(&task.id, "ch 1").unwrap();
    app.add_subtask(&task.id, "  ch 2 ").unwrap();
    assert!(matches!(app.add_subtask(&task.id, " "), Err(AppError::Validation(_))));
    assert!(matches!(app.add_subtask("missing", "x"), Err(AppError::NotFound(_))));

    // Done: no new subtasks, but the existing ones are kept.
    app.advance_task(&task.id).unwrap();
    assert!(matches!(app.add_subtask(&task.id, "ch 3"), Err(AppError::Validation(_))));
    let subs = &app.board().unwrap().lists[0].tasks[0].subtasks;
    assert_eq!(subs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["ch 1", "ch 2"]);
}

fn subtask_lifecycle<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    let task = app.add_task(&list, "book").unwrap();
    app.advance_task(&task.id).unwrap();
    let a = app.add_subtask(&task.id, "a").unwrap();
    let b = app.add_subtask(&task.id, "b").unwrap();

    assert!(app.toggle_subtask(&a.id).unwrap());
    app.edit_subtask(&b.id, "b2").unwrap();
    let subs = app.board().unwrap().lists[0].tasks[0].subtasks.clone();
    assert_eq!((subs[0].done, subs[1].text.as_str()), (true, "b2"));
    assert!(!app.toggle_subtask(&a.id).unwrap());

    app.delete_subtask(&a.id).unwrap();
    assert_eq!(app.board().unwrap().lists[0].tasks[0].subtasks.len(), 1);
    assert!(matches!(app.toggle_subtask(&a.id), Err(AppError::NotFound(_))));
}

fn deleting_a_task_or_list_deletes_subtasks<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    let t1 = app.add_task(&list, "one").unwrap();
    let t2 = app.add_task(&list, "two").unwrap();
    for t in [&t1, &t2] {
        app.advance_task(&t.id).unwrap();
        app.add_subtask(&t.id, "step").unwrap();
    }
    let step = app.board().unwrap().lists[0].tasks[0].subtasks[0].id.clone();

    app.delete_task(&t1.id).unwrap();
    assert!(matches!(app.toggle_subtask(&step), Err(AppError::NotFound(_))));

    let other = app.board().unwrap().lists[0].tasks[0].subtasks[0].id.clone();
    app.delete_list(&list).unwrap();
    assert!(matches!(app.toggle_subtask(&other), Err(AppError::NotFound(_))));
}

fn import_keeps_subtasks<S: Store>(app: UseCases<S>) {
    app.import_notes("- SWE book *\n  - / ch 1\n  - ch 2\n- plain").unwrap();
    let tasks = &app.board().unwrap().lists[0].tasks;
    assert_eq!(tasks[0].subtasks.len(), 2);
    assert!(tasks[0].subtasks[0].done);
    assert!(!tasks[0].subtasks[1].done);
    assert!(tasks[1].subtasks.is_empty());
}

fn completed_feed_is_newest_first_and_reopening_removes_items<S: Store>(app: UseCases<S>) {
    use tadah::application::CompletedKind::{Subtask, Task};

    let list = app.create_list("Work").unwrap();
    let a = app.add_task(&list, "a").unwrap();
    let b = app.add_task(&list, "b").unwrap();
    let c = app.add_task(&list, "c").unwrap();
    assert!(app.completed().unwrap().is_empty());

    app.advance_task(&b.id).unwrap(); // partial
    let step = app.add_subtask(&b.id, "step").unwrap();

    // The clock ticks on every call, so completion order = call order.
    app.set_task_status(&a.id, Status::Done).unwrap();
    app.set_subtask_done(&step.id, true).unwrap();
    app.set_task_status(&c.id, Status::Done).unwrap();

    let feed = app.completed().unwrap();
    let shown: Vec<_> = feed.iter().map(|i| (i.kind, i.text.as_str())).collect();
    assert_eq!(shown, [(Task, "c"), (Subtask, "step"), (Task, "a")]);
    assert!(feed[0].completed_at > feed[1].completed_at && feed[1].completed_at > feed[2].completed_at);

    // A subtask reports where it lives; the not-done parent "b" is not in the feed.
    assert_eq!(feed[1].parent_task.as_deref(), Some("b"));
    assert_eq!(feed[1].section, "Work");
    assert_eq!(feed[0].parent_task, None);

    // Uncomplete a task and a subtask: both leave the feed.
    app.set_task_status(&c.id, Status::Todo).unwrap();
    app.set_subtask_done(&step.id, false).unwrap();
    let shown: Vec<_> = app.completed().unwrap().into_iter().map(|i| i.text).collect();
    assert_eq!(shown, ["a"]);

    // Completing again records a fresh, later time.
    let first = app.completed().unwrap()[0].completed_at;
    app.set_task_status(&c.id, Status::Done).unwrap();
    let feed = app.completed().unwrap();
    assert_eq!(feed[0].text, "c");
    assert!(feed[0].completed_at > first);
}

fn completed_survives_being_cycled_and_leaves_with_deletes<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    let t = app.add_task(&list, "t").unwrap();
    app.advance_task(&t.id).unwrap();
    app.advance_task(&t.id).unwrap(); // done via the click cycle
    assert_eq!(app.completed().unwrap().len(), 1);
    app.advance_task(&t.id).unwrap(); // back to todo
    assert!(app.completed().unwrap().is_empty());

    app.set_task_status(&t.id, Status::Done).unwrap();
    app.delete_list(&list).unwrap();
    assert!(app.completed().unwrap().is_empty());
}

fn board_carries_creation_times<S: Store>(app: UseCases<S>) {
    let list = app.create_list("L").unwrap();
    let first = app.add_task(&list, "first").unwrap();
    let second = app.add_task(&list, "second").unwrap();
    app.advance_task(&first.id).unwrap();
    app.add_subtask(&first.id, "step").unwrap();
    // Editing or completing later must not change when something was added.
    app.edit_task(&first.id, "first, renamed").unwrap();
    app.set_task_status(&second.id, Status::Done).unwrap();

    let tasks = &app.board().unwrap().lists[0].tasks;
    let (a, b) = (&tasks[0], &tasks[1]);
    assert_eq!(a.created_at, first.created_at);
    assert_eq!(b.created_at, second.created_at);
    assert!(a.created_at > 0 && a.created_at < b.created_at);
    // The subtask was added after its parent.
    assert!(a.subtasks[0].created_at > a.created_at);
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => {
        mod memory {
            use super::*;
            $(#[test] fn $name() { super::$name(app(MemoryStore::default())) })*
        }
        mod sqlite {
            use super::*;
            $(#[test] fn $name() { super::$name(app(SqliteStore::open_in_memory().unwrap())) })*
        }
    };
}

scenarios!(
    add_edit_advance_delete,
    keeps_insertion_order,
    rejects_bad_input,
    deleting_a_list_deletes_its_tasks,
    import_appends_and_keeps_statuses,
    import_is_all_or_nothing,
    subtasks_only_on_partial_tasks,
    subtask_lifecycle,
    deleting_a_task_or_list_deletes_subtasks,
    import_keeps_subtasks,
    completed_feed_is_newest_first_and_reopening_removes_items,
    completed_survives_being_cycled_and_leaves_with_deletes,
    board_carries_creation_times,
);

#[test]
fn sqlite_persists_across_reopen() {
    let dir = std::env::temp_dir().join(format!("tadah-test-{}", std::process::id()));
    let path = dir.join("tadah.db");
    let _ = std::fs::remove_dir_all(&dir);

    let list = {
        let app = app(SqliteStore::open(&path).unwrap());
        let list = app.create_list("Kept").unwrap();
        app.add_task(&list, "still here").unwrap();
        list
    };

    let app = app(SqliteStore::open(&path).unwrap());
    let board = app.board().unwrap();
    assert_eq!(board.lists[0].id, list);
    assert_eq!(board.lists[0].tasks[0].text, "still here");
    let _ = std::fs::remove_dir_all(&dir);
}
