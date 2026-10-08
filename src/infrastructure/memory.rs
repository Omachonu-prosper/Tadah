use std::cell::RefCell;

use crate::application::ports::{ListRepository, Result, Store, SubtaskRepository, TaskRepository};
use crate::domain::{Subtask, Task, TodoList};

#[derive(Clone, Default)]
struct State {
    lists: Vec<TodoList>,
    tasks: Vec<Task>,
    subtasks: Vec<Subtask>,
}

/// In-memory adapter for tests: proves the core runs with no database.
#[derive(Default)]
pub struct MemoryStore {
    state: RefCell<State>,
}

impl ListRepository for MemoryStore {
    fn lists(&self) -> Result<Vec<TodoList>> {
        let mut lists = self.state.borrow().lists.clone();
        lists.sort_by_key(|l| l.position);
        Ok(lists)
    }

    fn list(&self, id: &str) -> Result<Option<TodoList>> {
        Ok(self.state.borrow().lists.iter().find(|l| l.id == id).cloned())
    }

    fn next_list_position(&self) -> Result<i64> {
        Ok(self.state.borrow().lists.iter().map(|l| l.position).max().map_or(0, |p| p + 1))
    }

    fn insert_list(&self, list: &TodoList) -> Result<()> {
        self.state.borrow_mut().lists.push(list.clone());
        Ok(())
    }

    fn update_list(&self, list: &TodoList) -> Result<()> {
        if let Some(l) = self.state.borrow_mut().lists.iter_mut().find(|l| l.id == list.id) {
            *l = list.clone();
        }
        Ok(())
    }

    fn delete_list(&self, id: &str) -> Result<()> {
        let mut s = self.state.borrow_mut();
        let doomed: Vec<String> =
            s.tasks.iter().filter(|t| t.list_id == id).map(|t| t.id.clone()).collect();
        s.lists.retain(|l| l.id != id);
        s.tasks.retain(|t| t.list_id != id);
        s.subtasks.retain(|sub| !doomed.contains(&sub.task_id));
        Ok(())
    }
}

impl TaskRepository for MemoryStore {
    fn tasks(&self) -> Result<Vec<Task>> {
        let mut tasks = self.state.borrow().tasks.clone();
        tasks.sort_by_key(|t| t.position);
        Ok(tasks)
    }

    fn task(&self, id: &str) -> Result<Option<Task>> {
        Ok(self.state.borrow().tasks.iter().find(|t| t.id == id).cloned())
    }

    fn next_task_position(&self, list_id: &str) -> Result<i64> {
        Ok(self
            .state
            .borrow()
            .tasks
            .iter()
            .filter(|t| t.list_id == list_id)
            .map(|t| t.position)
            .max()
            .map_or(0, |p| p + 1))
    }

    fn insert_task(&self, task: &Task) -> Result<()> {
        self.state.borrow_mut().tasks.push(task.clone());
        Ok(())
    }

    fn update_task(&self, task: &Task) -> Result<()> {
        if let Some(t) = self.state.borrow_mut().tasks.iter_mut().find(|t| t.id == task.id) {
            *t = task.clone();
        }
        Ok(())
    }

    fn delete_task(&self, id: &str) -> Result<()> {
        let mut s = self.state.borrow_mut();
        s.tasks.retain(|t| t.id != id);
        s.subtasks.retain(|sub| sub.task_id != id);
        Ok(())
    }
}

impl SubtaskRepository for MemoryStore {
    fn subtasks(&self) -> Result<Vec<Subtask>> {
        let mut subtasks = self.state.borrow().subtasks.clone();
        subtasks.sort_by_key(|s| s.position);
        Ok(subtasks)
    }

    fn subtask(&self, id: &str) -> Result<Option<Subtask>> {
        Ok(self.state.borrow().subtasks.iter().find(|s| s.id == id).cloned())
    }

    fn next_subtask_position(&self, task_id: &str) -> Result<i64> {
        Ok(self
            .state
            .borrow()
            .subtasks
            .iter()
            .filter(|s| s.task_id == task_id)
            .map(|s| s.position)
            .max()
            .map_or(0, |p| p + 1))
    }

    fn insert_subtask(&self, subtask: &Subtask) -> Result<()> {
        self.state.borrow_mut().subtasks.push(subtask.clone());
        Ok(())
    }

    fn update_subtask(&self, subtask: &Subtask) -> Result<()> {
        if let Some(s) = self.state.borrow_mut().subtasks.iter_mut().find(|s| s.id == subtask.id) {
            *s = subtask.clone();
        }
        Ok(())
    }

    fn delete_subtask(&self, id: &str) -> Result<()> {
        self.state.borrow_mut().subtasks.retain(|s| s.id != id);
        Ok(())
    }
}

impl Store for MemoryStore {
    fn transaction<T>(&self, work: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        let snapshot = self.state.borrow().clone();
        let result = work(self);
        if result.is_err() {
            *self.state.borrow_mut() = snapshot;
        }
        result
    }
}
