//! egui front end. Talks to the application only through `UseCases`.

use std::collections::HashMap;

use eframe::egui::{self, Color32, RichText, TextEdit};
use tadah::application::ports::Store;
use tadah::application::{Board, CompletedItem, CompletedKind, UseCases, notes};
use tadah::domain::Status;
use tadah::timefmt::format_local;

const MAX_WIDTH: f32 = 680.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Todo,
    Completed,
}

enum Action {
    /// Mark a finished task or subtask as not done again.
    Reopen(CompletedKind, String),
    Advance(String),
    EditTask(String, String),
    DeleteTask(String),
    AddTask(String, String),
    AddSubtask(String, String),
    ToggleSubtask(String),
    EditSubtask(String, String),
    DeleteSubtask(String),
    RenameList(String, String),
    DeleteList(String),
    CreateList(String),
    Import(String),
}

pub struct TadahApp<S: Store> {
    uc: UseCases<S>,
    board: Board,
    completed: Vec<CompletedItem>,
    tab: Tab,
    error: Option<String>,
    /// In-progress edits, keyed by task/list id. Only kept while the field has focus.
    drafts: HashMap<String, String>,
    new_task: HashMap<String, String>,
    new_list: String,
    import_open: bool,
    import_text: String,
    confirm_delete: Option<String>,
}

impl<S: Store> TadahApp<S> {
    pub fn new(uc: UseCases<S>) -> Self {
        let mut app = Self {
            uc,
            board: Board::default(),
            completed: Vec::new(),
            tab: Tab::Todo,
            error: None,
            drafts: HashMap::new(),
            new_task: HashMap::new(),
            new_list: String::new(),
            import_open: false,
            import_text: String::new(),
            confirm_delete: None,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match self.uc.board() {
            Ok(board) => self.board = board,
            Err(e) => self.error = Some(e.to_string()),
        }
        match self.uc.completed() {
            Ok(items) => self.completed = items,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn apply(&mut self, action: Action) {
        let result = match action {
            Action::Reopen(CompletedKind::Task, id) => self.uc.set_task_status(&id, Status::Todo),
            Action::Reopen(CompletedKind::Subtask, id) => self.uc.set_subtask_done(&id, false),
            Action::Advance(id) => self.uc.advance_task(&id).map(drop),
            Action::EditTask(id, text) => self.uc.edit_task(&id, &text),
            Action::DeleteTask(id) => self.uc.delete_task(&id),
            Action::AddTask(list, text) => self.uc.add_task(&list, &text).map(drop),
            Action::AddSubtask(task, text) => self.uc.add_subtask(&task, &text).map(drop),
            Action::ToggleSubtask(id) => self.uc.toggle_subtask(&id).map(drop),
            Action::EditSubtask(id, text) => self.uc.edit_subtask(&id, &text),
            Action::DeleteSubtask(id) => self.uc.delete_subtask(&id),
            Action::RenameList(id, title) => self.uc.rename_list(&id, &title),
            Action::DeleteList(id) => self.uc.delete_list(&id),
            Action::CreateList(title) => self.uc.create_list(&title).map(drop),
            Action::Import(text) => self.uc.import_notes(&text).map(drop),
        };
        match result {
            Ok(()) => self.error = None,
            Err(e) => self.error = Some(e.to_string()),
        }
        self.refresh();
    }
}

fn status_color(status: Status, ui: &egui::Ui) -> Color32 {
    match status {
        Status::Done => Color32::from_rgb(0x4a, 0xb0, 0x6a),
        Status::Partial => Color32::from_rgb(0xe0, 0x9a, 0x1f),
        Status::Todo => ui.visuals().weak_text_color(),
    }
}

fn marker(status: Status) -> &'static str {
    match status {
        Status::Todo => "-",
        Status::Partial => "*",
        Status::Done => "/",
    }
}

/// A frameless single-line field that edits a local draft and reports the new value
/// when it loses focus (Enter or click-away). Returns `Some(text)` only if it changed.
fn draft_field(
    ui: &mut egui::Ui,
    drafts: &mut HashMap<String, String>,
    id: &str,
    current: &str,
    allow_empty: bool,
    hint: &str,
    color: Option<Color32>,
    tooltip: Option<String>,
) -> Option<String> {
    let mut text = drafts.get(id).cloned().unwrap_or_else(|| current.to_string());
    let mut edit = TextEdit::singleline(&mut text)
        .frame(egui::Frame::NONE)
        .desired_width(f32::INFINITY)
        .hint_text(hint);
    if let Some(c) = color {
        edit = edit.text_color(c);
    }
    let mut response = ui.add(edit);
    if let Some(text) = tooltip {
        response = response.on_hover_text(text);
    }

    if response.lost_focus() {
        drafts.remove(id);
        let trimmed = text.trim();
        // An emptied task would be rejected; snap back instead.
        if trimmed != current && (allow_empty || !trimmed.is_empty()) {
            return Some(text);
        }
    } else if response.has_focus() || response.changed() {
        drafts.insert(id.to_string(), text);
    } else {
        drafts.remove(id);
    }
    None
}

/// A frameless "+ add ..." field. Returns the text when Enter is pressed, and keeps focus
/// so several items can be typed in a row.
fn enter_field(ui: &mut egui::Ui, buf: &mut String, hint: &str) -> Option<String> {
    let response = ui.add(
        TextEdit::singleline(buf)
            .frame(egui::Frame::NONE)
            .desired_width(f32::INFINITY)
            .hint_text(hint),
    );
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        response.request_focus();
        let text = std::mem::take(buf);
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

impl<S: Store> eframe::App for TadahApp<S> {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions: Vec<Action> = Vec::new();
        let ctx = ui.ctx().clone();

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let width = ui.available_width().min(MAX_WIDTH);
                let pad = (ui.available_width() - width) / 2.0;
                ui.horizontal_top(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        self.content(ui, &ctx, &mut actions);
                    });
                });
            });
        });

        for action in actions {
            self.apply(action);
        }
    }
}

impl<S: Store> TadahApp<S> {
    fn content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, actions: &mut Vec<Action>) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.heading("Tadah");
            ui.label(
                RichText::new(format!(
                    "{} open · {} partial · {} done",
                    self.board.count(Status::Todo),
                    self.board.count(Status::Partial),
                    self.board.count(Status::Done)
                ))
                .weak(),
            );
            if self.tab == Tab::Todo {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.import_open { "close" } else { "import / export" };
                    if ui.link(label).clicked() {
                        self.import_open = !self.import_open;
                    }
                });
            }
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Todo, "Todo");
            let label = format!("Completed ({})", self.completed.len());
            ui.selectable_value(&mut self.tab, Tab::Completed, label);
        });
        ui.separator();

        if let Some(err) = &self.error {
            ui.colored_label(Color32::from_rgb(0xdd, 0x4b, 0x4b), err);
            ui.add_space(8.0);
        }

        if self.tab == Tab::Completed {
            self.completed_view(ui, actions);
            return;
        }

        if self.import_open {
            self.import_panel(ui, ctx, actions);
        }

        for list in &self.board.lists {
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                if let Some(title) = draft_field(
                    ui,
                    &mut self.drafts,
                    &list.id,
                    &list.title,
                    true,
                    "Untitled section",
                    Some(ui.visuals().strong_text_color()),
                    None,
                ) {
                    actions.push(Action::RenameList(list.id.clone(), title));
                }
            });
            // Section delete sits below the title so the title field can span the row.
            ui.horizontal(|ui| {
                if self.confirm_delete.as_deref() == Some(list.id.as_str()) {
                    ui.label(format!("Delete this section and its {} tasks?", list.tasks.len()));
                    if ui.small_button("Yes, delete").clicked() {
                        actions.push(Action::DeleteList(list.id.clone()));
                        self.confirm_delete = None;
                    }
                    if ui.small_button("Cancel").clicked() {
                        self.confirm_delete = None;
                    }
                } else if ui.small_button("delete section").clicked() {
                    if list.tasks.is_empty() {
                        actions.push(Action::DeleteList(list.id.clone()));
                    } else {
                        self.confirm_delete = Some(list.id.clone());
                    }
                }
            });

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                // Finished tasks live in the Completed tab.
                for task in list.tasks.iter().filter(|t| t.status != Status::Done) {
                    ui.horizontal(|ui| {
                        let color = status_color(task.status, ui);
                        let button = egui::Button::new(
                            RichText::new(marker(task.status)).monospace().strong().color(color),
                        )
                        .min_size(egui::vec2(26.0, 26.0));
                        if ui.add(button).on_hover_text(status_label(task.status)).clicked() {
                            actions.push(Action::Advance(task.id.clone()));
                        }

                        let text_color =
                            (task.status == Status::Done).then(|| ui.visuals().weak_text_color());
                        let has_subtasks = !task.subtasks.is_empty();
                        let field_width =
                            ui.available_width() - 30.0 - if has_subtasks { 48.0 } else { 0.0 };
                        ui.allocate_ui(egui::vec2(field_width, 26.0), |ui| {
                            if let Some(text) = draft_field(
                                ui,
                                &mut self.drafts,
                                &task.id,
                                &task.text,
                                false,
                                "",
                                text_color,
                                Some(format!("Added {}", format_local(task.created_at))),
                            ) {
                                actions.push(Action::EditTask(task.id.clone(), text));
                            }
                        });

                        if has_subtasks {
                            let done = task.subtasks.iter().filter(|s| s.done).count();
                            ui.label(
                                RichText::new(format!("{done}/{}", task.subtasks.len()))
                                    .weak()
                                    .small(),
                            );
                        }

                        if ui.small_button("×").on_hover_text("Delete task").clicked() {
                            actions.push(Action::DeleteTask(task.id.clone()));
                        }
                    });

                    // Steps under a task. New ones can only be added while it's partly done.
                    if has_subtasks_or_can_add(task) {
                        ui.indent(("subtasks", &task.id), |ui| {
                            for sub in &task.subtasks {
                                ui.horizontal(|ui| {
                                    let mut done = sub.done;
                                    if ui.checkbox(&mut done, "").changed() {
                                        actions.push(Action::ToggleSubtask(sub.id.clone()));
                                    }
                                    let color =
                                        sub.done.then(|| ui.visuals().weak_text_color());
                                    let field_width = ui.available_width() - 30.0;
                                    ui.allocate_ui(egui::vec2(field_width, 22.0), |ui| {
                                        if let Some(text) = draft_field(
                                            ui,
                                            &mut self.drafts,
                                            &sub.id,
                                            &sub.text,
                                            false,
                                            "",
                                            color,
                                            Some(format!("Added {}", format_local(sub.created_at))),
                                        ) {
                                            actions.push(Action::EditSubtask(sub.id.clone(), text));
                                        }
                                    });
                                    if ui.small_button("×").on_hover_text("Delete subtask").clicked()
                                    {
                                        actions.push(Action::DeleteSubtask(sub.id.clone()));
                                    }
                                });
                            }
                            if task.accepts_subtasks() {
                                let draft = self.new_task.entry(task.id.clone()).or_default();
                                if let Some(text) = enter_field(ui, draft, "+ add subtask") {
                                    actions.push(Action::AddSubtask(task.id.clone(), text));
                                }
                            }
                        });
                    }
                }

                let draft = self.new_task.entry(list.id.clone()).or_default();
                if let Some(text) = enter_field(ui, draft, "+ add task") {
                    actions.push(Action::AddTask(list.id.clone(), text));
                }
            });
        }

        ui.add_space(20.0);
        let hint = if self.board.lists.is_empty() {
            "+ start your first list (press Enter)"
        } else {
            "+ new section (press Enter)"
        };
        if let Some(title) = enter_field(ui, &mut self.new_list, hint) {
            actions.push(Action::CreateList(title));
        }
        ui.add_space(24.0);
    }

    /// Finished tasks and subtasks, newest first, each with when it was completed.
    fn completed_view(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.completed.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("Nothing completed yet.").weak());
            return;
        }

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, item) in self.completed.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    if ui.small_button("Reopen").on_hover_text("Mark as not done").clicked() {
                        actions.push(Action::Reopen(item.kind, item.id.clone()));
                    }
                    let (marker, done_color) = match item.kind {
                        CompletedKind::Task => ("/", status_color(Status::Done, ui)),
                        CompletedKind::Subtask => ("[x]", status_color(Status::Done, ui)),
                    };
                    ui.label(RichText::new(marker).monospace().strong().color(done_color));
                    ui.vertical(|ui| {
                        ui.label(&item.text);
                        let mut details = vec![format_local(item.completed_at)];
                        if let Some(parent) = &item.parent_task {
                            details.push(format!("under: {parent}"));
                        }
                        if !item.section.is_empty() {
                            details.push(item.section.clone());
                        }
                        ui.label(RichText::new(details.join("  ·  ")).weak().small());
                    });
                });
            }
        });
        ui.add_space(24.0);
    }

    fn import_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(
                    "Notes format: \"- task\", \"- * partly done\", \"- / done\". \
                     Lines without a dash start a new section. Import appends to your lists.",
                )
                .weak()
                .small(),
            );
            ui.add(
                TextEdit::multiline(&mut self.import_text)
                    .desired_rows(10)
                    .desired_width(f32::INFINITY)
                    .code_editor(),
            );
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() && !self.import_text.trim().is_empty() {
                    actions.push(Action::Import(std::mem::take(&mut self.import_text)));
                    self.import_open = false;
                }
                if ui.button("Copy all as text").clicked() {
                    ctx.copy_text(notes::serialize(&self.board));
                }
            });
        });
    }
}

fn has_subtasks_or_can_add(task: &tadah::application::TaskView) -> bool {
    !task.subtasks.is_empty() || task.accepts_subtasks()
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Todo => "Not started — click to change",
        Status::Partial => "Partly done — click to change",
        Status::Done => "Done — click to change",
    }
}
