//! Ports: defined here, implemented by outer circles.

use super::AppError;
use crate::domain::{Subtask, Task, TodoList};

pub type Result<T> = std::result::Result<T, AppError>;

pub trait ListRepository {
    /// Ordered by position.
    fn lists(&self) -> Result<Vec<TodoList>>;
    fn list(&self, id: &str) -> Result<Option<TodoList>>;
    /// Position for a list appended after all existing ones.
    fn next_list_position(&self) -> Result<i64>;
    fn insert_list(&self, list: &TodoList) -> Result<()>;
    fn update_list(&self, list: &TodoList) -> Result<()>;
    /// Deletes the list and every task (and subtask) in it.
    fn delete_list(&self, id: &str) -> Result<()>;
}

pub trait TaskRepository {
    /// Ordered by list, then position.
    fn tasks(&self) -> Result<Vec<Task>>;
    fn task(&self, id: &str) -> Result<Option<Task>>;
    /// Position for a task appended after all existing ones in the list.
    fn next_task_position(&self, list_id: &str) -> Result<i64>;
    fn insert_task(&self, task: &Task) -> Result<()>;
    fn update_task(&self, task: &Task) -> Result<()>;
    fn delete_task(&self, id: &str) -> Result<()>;
}

pub trait SubtaskRepository {
    /// Ordered by task, then position.
    fn subtasks(&self) -> Result<Vec<Subtask>>;
    fn subtask(&self, id: &str) -> Result<Option<Subtask>>;
    /// Position for a subtask appended after all existing ones under the task.
    fn next_subtask_position(&self, task_id: &str) -> Result<i64>;
    fn insert_subtask(&self, subtask: &Subtask) -> Result<()>;
    fn update_subtask(&self, subtask: &Subtask) -> Result<()>;
    fn delete_subtask(&self, id: &str) -> Result<()>;
}

pub trait Store: ListRepository + TaskRepository + SubtaskRepository {
    /// Runs `work` atomically: everything commits together or not at all.
    fn transaction<T>(&self, work: impl FnOnce(&Self) -> Result<T>) -> Result<T>;
}

pub trait Clock {
    /// Unix seconds.
    fn now(&self) -> i64;
}

pub trait IdGenerator {
    fn next(&self) -> String;
}
