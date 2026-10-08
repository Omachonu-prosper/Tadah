use super::notes;
use super::ports::{Clock, IdGenerator, Result, Store};
use super::{AppError, Board, CompletedItem, CompletedKind, ListView, SubtaskView, TaskView};
use crate::domain::{Status, Subtask, Task, TodoList, normalize_text};

/// One method per application operation; each runs in a single transaction.
pub struct UseCases<S: Store> {
    store: S,
    clock: Box<dyn Clock>,
    ids: Box<dyn IdGenerator>,
}

fn subtask_view(s: &Subtask) -> SubtaskView {
    SubtaskView { id: s.id.clone(), text: s.text.clone(), done: s.done, created_at: s.created_at }
}

fn task_view(t: &Task, subtasks: &[Subtask]) -> TaskView {
    TaskView {
        id: t.id.clone(),
        text: t.text.clone(),
        status: t.status,
        created_at: t.created_at,
        subtasks: subtasks.iter().filter(|s| s.task_id == t.id).map(subtask_view).collect(),
    }
}

fn not_found<T>(what: &str) -> Result<T> {
    Err(AppError::NotFound(what.into()))
}

impl<S: Store> UseCases<S> {
    pub fn new(store: S, clock: Box<dyn Clock>, ids: Box<dyn IdGenerator>) -> Self {
        Self { store, clock, ids }
    }

    pub fn board(&self) -> Result<Board> {
        self.store.transaction(|s| {
            let tasks = s.tasks()?;
            let subtasks = s.subtasks()?;
            let lists = s
                .lists()?
                .into_iter()
                .map(|l| ListView {
                    tasks: tasks
                        .iter()
                        .filter(|t| t.list_id == l.id)
                        .map(|t| task_view(t, &subtasks))
                        .collect(),
                    id: l.id,
                    title: l.title,
                })
                .collect();
            Ok(Board { lists })
        })
    }

    /// Every finished task and subtask, most recently completed first. Items finished at
    /// the same moment keep their board order. Uncompleting one (`set_task_status`,
    /// `set_subtask_done`) removes it from this list.
    pub fn completed(&self) -> Result<Vec<CompletedItem>> {
        self.store.transaction(|s| {
            let tasks = s.tasks()?;
            let subtasks = s.subtasks()?;
            let mut items = Vec::new();
            for list in s.lists()? {
                for task in tasks.iter().filter(|t| t.list_id == list.id) {
                    if let Some(at) = task.completed_at {
                        items.push(CompletedItem {
                            id: task.id.clone(),
                            kind: CompletedKind::Task,
                            text: task.text.clone(),
                            section: list.title.clone(),
                            parent_task: None,
                            completed_at: at,
                        });
                    }
                    for sub in subtasks.iter().filter(|x| x.task_id == task.id) {
                        if let Some(at) = sub.completed_at {
                            items.push(CompletedItem {
                                id: sub.id.clone(),
                                kind: CompletedKind::Subtask,
                                text: sub.text.clone(),
                                section: list.title.clone(),
                                parent_task: Some(task.text.clone()),
                                completed_at: at,
                            });
                        }
                    }
                }
            }
            // Stable sort: ties keep the board order built above.
            items.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
            Ok(items)
        })
    }

    pub fn create_list(&self, title: &str) -> Result<String> {
        self.store.transaction(|s| {
            let list = TodoList::new(
                self.ids.next(),
                title,
                s.next_list_position()?,
                self.clock.now(),
            )?;
            s.insert_list(&list)?;
            Ok(list.id)
        })
    }

    pub fn rename_list(&self, list_id: &str, title: &str) -> Result<()> {
        self.store.transaction(|s| {
            let Some(mut list) = s.list(list_id)? else { return not_found("List") };
            list.rename(title, self.clock.now())?;
            s.update_list(&list)
        })
    }

    pub fn delete_list(&self, list_id: &str) -> Result<()> {
        self.store.transaction(|s| s.delete_list(list_id))
    }

    pub fn add_task(&self, list_id: &str, text: &str) -> Result<TaskView> {
        self.store.transaction(|s| {
            if s.list(list_id)?.is_none() {
                return not_found("List");
            }
            let task = Task::new(
                self.ids.next(),
                list_id.to_string(),
                text,
                Status::Todo,
                s.next_task_position(list_id)?,
                self.clock.now(),
            )?;
            s.insert_task(&task)?;
            Ok(task_view(&task, &[]))
        })
    }

    pub fn edit_task(&self, task_id: &str, text: &str) -> Result<()> {
        self.store.transaction(|s| {
            let Some(mut task) = s.task(task_id)? else { return not_found("Task") };
            task.rename(text, self.clock.now())?;
            s.update_task(&task)
        })
    }

    /// Todo -> Partial -> Done -> Todo. Existing subtasks are kept whatever the new status.
    pub fn advance_task(&self, task_id: &str) -> Result<Status> {
        self.store.transaction(|s| {
            let Some(mut task) = s.task(task_id)? else { return not_found("Task") };
            task.advance(self.clock.now());
            s.update_task(&task)?;
            Ok(task.status)
        })
    }

    pub fn set_task_status(&self, task_id: &str, status: Status) -> Result<()> {
        self.store.transaction(|s| {
            let Some(mut task) = s.task(task_id)? else { return not_found("Task") };
            task.set_status(status, self.clock.now());
            s.update_task(&task)
        })
    }

    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        self.store.transaction(|s| s.delete_task(task_id))
    }

    /// Only partly done tasks accept new subtasks.
    pub fn add_subtask(&self, task_id: &str, text: &str) -> Result<SubtaskView> {
        self.store.transaction(|s| {
            let Some(task) = s.task(task_id)? else { return not_found("Task") };
            task.ensure_accepts_subtasks()?;
            let subtask = Subtask::new(
                self.ids.next(),
                task_id.to_string(),
                text,
                false,
                s.next_subtask_position(task_id)?,
                self.clock.now(),
            )?;
            s.insert_subtask(&subtask)?;
            Ok(subtask_view(&subtask))
        })
    }

    pub fn toggle_subtask(&self, subtask_id: &str) -> Result<bool> {
        self.store.transaction(|s| {
            let Some(mut sub) = s.subtask(subtask_id)? else { return not_found("Subtask") };
            sub.toggle(self.clock.now());
            s.update_subtask(&sub)?;
            Ok(sub.done)
        })
    }

    pub fn set_subtask_done(&self, subtask_id: &str, done: bool) -> Result<()> {
        self.store.transaction(|s| {
            let Some(mut sub) = s.subtask(subtask_id)? else { return not_found("Subtask") };
            sub.set_done(done, self.clock.now());
            s.update_subtask(&sub)
        })
    }

    pub fn edit_subtask(&self, subtask_id: &str, text: &str) -> Result<()> {
        self.store.transaction(|s| {
            let Some(mut sub) = s.subtask(subtask_id)? else { return not_found("Subtask") };
            sub.rename(text, self.clock.now())?;
            s.update_subtask(&sub)
        })
    }

    pub fn delete_subtask(&self, subtask_id: &str) -> Result<()> {
        self.store.transaction(|s| s.delete_subtask(subtask_id))
    }

    /// Appends lists, tasks and subtasks parsed from plain-text notes; all-or-nothing.
    /// Returns (lists, tasks) added.
    pub fn import_notes(&self, text: &str) -> Result<(usize, usize)> {
        let parsed = notes::parse(text);
        // Validate everything before writing anything.
        for list in &parsed {
            for task in &list.tasks {
                normalize_text(&task.text)?;
                for (sub, _) in &task.subtasks {
                    normalize_text(sub)?;
                }
            }
        }

        self.store.transaction(|s| {
            let now = self.clock.now();
            let first = s.next_list_position()?;
            let mut task_count = 0;
            for (i, p) in parsed.iter().enumerate() {
                let list = TodoList::new(self.ids.next(), &p.title, first + i as i64, now)?;
                s.insert_list(&list)?;
                for (j, t) in p.tasks.iter().enumerate() {
                    let task = Task::new(
                        self.ids.next(),
                        list.id.clone(),
                        &t.text,
                        t.status,
                        j as i64,
                        now,
                    )?;
                    s.insert_task(&task)?;
                    task_count += 1;
                    // Imported notes are taken as written: the "partly done only" rule
                    // governs adding subtasks interactively, not restoring existing ones.
                    for (k, (sub_text, done)) in t.subtasks.iter().enumerate() {
                        let sub = Subtask::new(
                            self.ids.next(),
                            task.id.clone(),
                            sub_text,
                            *done,
                            k as i64,
                            now,
                        )?;
                        s.insert_subtask(&sub)?;
                    }
                }
            }
            Ok((parsed.len(), task_count))
        })
    }
}
