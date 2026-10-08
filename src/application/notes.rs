//! The plain-text notes convention:
//!   "- task"        open
//!   "- * task"      partly done (a trailing "*" is accepted too)
//!   "- / task"      done
//!   "  - / step"    an indented line is a subtask of the task above ("/" = done)
//!   any other line  starts a new list

use super::Board;
use crate::domain::Status;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTask {
    pub text: String,
    pub status: Status,
    /// (text, done)
    pub subtasks: Vec<(String, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedList {
    pub title: String,
    pub tasks: Vec<ParsedTask>,
}

pub fn parse(text: &str) -> Vec<ParsedList> {
    let mut lists: Vec<ParsedList> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let Some(rest) = line.strip_prefix('-') else {
            lists.push(ParsedList { title: line.to_string(), tasks: Vec::new() });
            continue;
        };

        let indented = raw.starts_with([' ', '\t']);
        let mut body = rest.trim();
        let mut status = Status::Todo;
        if let Some(b) = body.strip_prefix('/') {
            status = Status::Done;
            body = b.trim();
        } else if let Some(b) = body.strip_prefix('*') {
            status = Status::Partial;
            body = b.trim();
        } else if let Some(b) = body.strip_suffix('*') {
            status = Status::Partial;
            body = b.trim();
        }
        if body.is_empty() {
            continue;
        }

        // An indented line belongs to the task above it, if there is one.
        if indented
            && let Some(task) = lists.last_mut().and_then(|l| l.tasks.last_mut())
        {
            task.subtasks.push((body.to_string(), status == Status::Done));
            continue;
        }

        if lists.is_empty() {
            lists.push(ParsedList { title: String::new(), tasks: Vec::new() });
        }
        lists.last_mut().unwrap().tasks.push(ParsedTask {
            text: body.to_string(),
            status,
            subtasks: Vec::new(),
        });
    }
    lists
}

pub fn serialize(board: &Board) -> String {
    board
        .lists
        .iter()
        .filter(|l| !l.title.is_empty() || !l.tasks.is_empty())
        .map(|l| {
            let mut lines = Vec::new();
            if !l.title.is_empty() {
                lines.push(l.title.clone());
            }
            for t in &l.tasks {
                let marker = match t.status {
                    Status::Todo => "",
                    Status::Partial => "* ",
                    Status::Done => "/ ",
                };
                lines.push(format!("- {marker}{}", t.text));
                for s in &t.subtasks {
                    lines.push(format!("  - {}{}", if s.done { "/ " } else { "" }, s.text));
                }
            }
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{ListView, SubtaskView, TaskView};

    const NOTES: &str = "- AfaraAI review\n- SWE book *\n  - / chapter 1\n  - chapter 2\n\nAfara.AI Todos\n- / Text replace\n- * Half done\n- Plain";

    #[test]
    fn parses_markers_and_sections() {
        let lists = parse(NOTES);
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].title, "");
        assert_eq!(lists[0].tasks[1].text, "SWE book");
        assert_eq!(lists[0].tasks[1].status, Status::Partial);
        assert_eq!(lists[1].title, "Afara.AI Todos");
        assert_eq!(lists[1].tasks[0].status, Status::Done);
        assert_eq!(lists[1].tasks[1].status, Status::Partial);
        assert_eq!(lists[1].tasks[2].status, Status::Todo);
    }

    #[test]
    fn indented_lines_become_subtasks() {
        let lists = parse(NOTES);
        assert_eq!(
            lists[0].tasks[1].subtasks,
            [("chapter 1".to_string(), true), ("chapter 2".to_string(), false)]
        );
        assert!(lists[0].tasks[0].subtasks.is_empty());
    }

    #[test]
    fn indented_line_with_no_task_above_is_a_task() {
        let lists = parse("  - orphan");
        assert_eq!(lists[0].tasks[0].text, "orphan");
    }

    #[test]
    fn skips_empty_tasks() {
        assert!(parse("-\n- /\n- *").is_empty());
    }

    #[test]
    fn serialize_round_trips() {
        let board = Board {
            lists: parse(NOTES)
                .into_iter()
                .enumerate()
                .map(|(i, l)| ListView {
                    id: i.to_string(),
                    title: l.title,
                    tasks: l
                        .tasks
                        .into_iter()
                        .map(|t| TaskView {
                            id: t.text.clone(),
                            text: t.text,
                            status: t.status,
                            created_at: 0,
                            subtasks: t
                                .subtasks
                                .into_iter()
                                .map(|(text, done)| SubtaskView { id: text.clone(), text, done, created_at: 0 })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        };
        let text = serialize(&board);
        assert_eq!(parse(&text), parse(NOTES));
    }
}
