mod compact;

use chrono::{Local, Utc};
use eframe::egui::{self, Color32, RichText};
use focus::{
    model::{Outcome, SessionKind, Settings, State, Task},
    storage::Storage,
    timer::Timer,
};
use std::{
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

pub(crate) const FULL_WINDOW_SIZE: egui::Vec2 = egui::vec2(1040.0, 860.0);
pub(crate) const FULL_MIN_SIZE: egui::Vec2 = egui::vec2(680.0, 560.0);

const ACCENT: Color32 = Color32::from_rgb(142, 218, 174);
const MUTED: Color32 = Color32::from_rgb(156, 170, 166);
const BACKGROUND: Color32 = Color32::from_rgb(18, 24, 24);
const SURFACE: Color32 = Color32::from_rgb(26, 34, 33);
const INSET: Color32 = Color32::from_rgb(21, 28, 27);
const BORDER: Color32 = Color32::from_rgb(46, 59, 55);
const TEXT: Color32 = Color32::from_rgb(233, 239, 235);
const SELECTED: Color32 = Color32::from_rgb(34, 53, 44);

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(18)
        .inner_margin(22)
}

fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).strong().color(MUTED));
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(color));
        });
}

pub struct FocusApp {
    storage: Storage,
    settings: Settings,
    draft: Settings,
    state: State,
    timer: Timer,
    audio: Option<PathBuf>,
    alerts: Vec<Receiver<String>>,
    new_task: String,
    editing: Option<(u64, String)>,
    deleting: Option<u64>,
    show_settings: bool,
    notice: String,
    error: Option<String>,
    dirty: bool,
    show_completed: bool,
    compact: bool,
    full_window_size: egui::Vec2,
    full_window_maximized: bool,
    pending_window_size: Option<egui::Vec2>,
}

impl FocusApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        storage: Storage,
        settings: Settings,
        state: State,
        audio: Option<PathBuf>,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = SURFACE;
        visuals.window_corner_radius = egui::CornerRadius::same(14);
        visuals.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
        visuals.override_text_color = Some(TEXT);
        visuals.extreme_bg_color = INSET;
        visuals.faint_bg_color = INSET;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 49, 45);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(38, 49, 45);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(49, 66, 57);
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(49, 66, 57);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
        visuals.widgets.active.bg_fill = SELECTED;
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.noninteractive,
        ] {
            widget.corner_radius = egui::CornerRadius::same(8);
        }
        visuals.selection.bg_fill = Color32::from_rgb(49, 89, 71);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(14.0, 9.0);
            style.spacing.interact_size.y = 34.0;
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        });
        let timer = Timer::new(settings.duration(state.next_kind));
        Self {
            storage,
            draft: settings.clone(),
            settings,
            state,
            timer,
            audio,
            alerts: Vec::new(),
            new_task: String::new(),
            editing: None,
            deleting: None,
            show_settings: false,
            notice: String::new(),
            error: None,
            dirty: false,
            show_completed: false,
            compact: false,
            full_window_size: FULL_WINDOW_SIZE,
            full_window_maximized: false,
            pending_window_size: None,
        }
    }

    fn persist(&mut self) -> bool {
        match self.storage.save_state(&self.state) {
            Ok(()) => {
                self.dirty = false;
                true
            }
            Err(error) => {
                self.dirty = true;
                self.error = Some(format!("Não foi possível salvar os dados: {error:#}"));
                false
            }
        }
    }

    fn timer_action_label(&self) -> &'static str {
        if self.timer.running() {
            "Pausar sessão"
        } else if self.timer.active() {
            "Retomar sessão"
        } else {
            "Iniciar sessão"
        }
    }

    fn toggle_timer(&mut self) {
        if self.dirty {
            return;
        }
        let now = Instant::now();
        let was_active = self.timer.active();
        self.advance(now);
        if self.dirty || (was_active && !self.timer.active()) {
            return;
        }
        if self.timer.running() {
            self.timer.pause(now);
        } else if self.timer.active() {
            self.timer.start(now);
        } else {
            let old_state = self.state.clone();
            self.state.begin(Utc::now(), &self.settings);
            if self.persist() {
                self.timer.start(Instant::now());
                self.notice.clear();
            } else {
                self.state = old_state;
            }
        }
    }

    fn error_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = self.error.clone() {
            ui.colored_label(Color32::from_rgb(250, 177, 145), error);
            if self.dirty {
                ui.label("Existem alterações sem salvar. Tente novamente antes de continuar.");
                if ui.button("Tentar salvar novamente").clicked() && self.persist() {
                    self.error = None;
                }
            } else if ui.small_button("Dispensar aviso").clicked() {
                self.error = None;
            }
            ui.separator();
        }
    }

    fn reset(&mut self) {
        if self.timer.active() {
            self.state.finish(Utc::now(), Outcome::Interrupted);
            self.notice = "Sessão interrompida. Nenhum pomodoro foi contabilizado.".into();
            self.persist();
        }
        self.timer = Timer::new(self.settings.duration(self.state.next_kind));
    }

    fn advance(&mut self, now: Instant) {
        let delay = self.timer.overdue(now);
        if self.timer.tick(now) {
            // Attribute completion to the actual deadline, including across midnight.
            let ended_at = Utc::now() - chrono::Duration::from_std(delay).unwrap_or_default();
            if let Some(kind) = self.state.finish(ended_at, Outcome::Completed) {
                self.notice = format!(
                    "{} concluído! Próxima sessão: {}. Inicie quando quiser.",
                    kind.label(),
                    self.state.next_kind.label().to_lowercase()
                );
                self.alerts.push(crate::alerts::notify(
                    self.notice.clone(),
                    self.settings.notifications,
                    self.settings.sound,
                    self.audio.clone(),
                ));
                self.persist();
            }
            self.timer = Timer::new(self.settings.duration(self.state.next_kind));
        }
    }

    fn timer_ui(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 10.0;
        card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(468.0);
            ui.add_enabled_ui(!self.dirty && !self.timer.active(), |ui| {
                egui::Frame::new()
                    .fill(INSET)
                    .corner_radius(10)
                    .inner_margin(4)
                    .show(ui, |ui| {
                        let width = (ui.available_width() - 8.0) / 3.0;
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.horizontal(|ui| {
                            for kind in [
                                SessionKind::Focus,
                                SessionKind::ShortBreak,
                                SessionKind::LongBreak,
                            ] {
                                let selected = self.state.next_kind == kind;
                                let button = egui::Button::new(
                                    RichText::new(kind.label()).size(12.0).color(if selected {
                                        ACCENT
                                    } else {
                                        MUTED
                                    }),
                                )
                                .fill(if selected {
                                    SELECTED
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(7);
                                if ui.add_sized([width, 32.0], button).clicked() {
                                    self.state.next_kind = kind;
                                    self.timer = Timer::new(self.settings.duration(kind));
                                    self.persist();
                                }
                            }
                        });
                    });
            });
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                let remaining = self.timer.remaining(Instant::now());
                let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
                let size = ui.available_width().min(232.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                let center = rect.center();
                let radius = size / 2.0 - 13.0;
                let painter = ui.painter();
                painter.circle_filled(center, radius - 10.0, INSET);
                painter.circle_stroke(center, radius, egui::Stroke::new(4.0_f32, BORDER));
                let fraction =
                    (remaining.as_secs_f32() / self.timer.duration().as_secs_f32()).clamp(0.0, 1.0);
                let points = (0..=120)
                    .map(|i| {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + std::f32::consts::TAU * fraction * i as f32 / 120.0;
                        center + egui::vec2(angle.cos(), angle.sin()) * radius
                    })
                    .collect();
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(4.0_f32, ACCENT),
                ));
                let status = if self.timer.running() {
                    "EM ANDAMENTO"
                } else if self.timer.active() {
                    "SESSÃO PAUSADA"
                } else {
                    "SEU MOMENTO DE FOCO"
                };
                let status = if !self.timer.active() && self.state.next_kind != SessionKind::Focus {
                    "HORA DE DESCANSAR"
                } else {
                    status
                };
                painter.text(
                    center - egui::vec2(0.0, 43.0),
                    egui::Align2::CENTER_CENTER,
                    status,
                    egui::FontId::proportional(10.0),
                    MUTED,
                );
                painter.text(
                    center + egui::vec2(0.0, 0.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{:02}:{:02}", seconds / 60, seconds % 60),
                    egui::FontId::proportional(size * 0.235),
                    TEXT,
                );
                painter.text(
                    center + egui::vec2(0.0, 45.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "{} minutos · {}",
                        self.timer.duration().as_secs() / 60,
                        self.state.next_kind.label().to_lowercase()
                    ),
                    egui::FontId::proportional(12.0),
                    MUTED,
                );
            });
            ui.add_space(8.0);
            let task = self
                .state
                .tasks
                .iter()
                .find(|t| Some(t.id) == self.state.selected_task);
            egui::Frame::new()
                .fill(INSET)
                .corner_radius(10)
                .inner_margin(14)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let focus = self.state.next_kind == SessionKind::Focus;
                    eyebrow(
                        ui,
                        if focus {
                            "TAREFA DA SESSÃO"
                        } else {
                            "FAÇA UMA PAUSA"
                        },
                    );
                    let title = if focus {
                        task.map_or("Foco livre", |t| t.title.as_str())
                    } else {
                        "Respire, alongue-se e descanse."
                    };
                    ui.add(
                        egui::Label::new(RichText::new(title).size(14.0).color(if focus {
                            ACCENT
                        } else {
                            TEXT
                        }))
                        .truncate(),
                    )
                    .on_hover_text(title);
                });
            ui.add_space(12.0);
            ui.add_enabled_ui(!self.dirty, |ui| {
                let label = self.timer_action_label();
                let gap = ui.spacing().item_spacing.x;
                let main_width = (ui.available_width() - gap) * 0.64;
                let reset_width = ui.available_width() - gap - main_width;
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [main_width, 44.0],
                            egui::Button::new(
                                RichText::new(label)
                                    .strong()
                                    .color(Color32::from_rgb(22, 43, 31)),
                            )
                            .fill(ACCENT)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(10),
                        )
                        .clicked()
                    {
                        self.toggle_timer();
                    }
                    if ui
                        .add_sized(
                            [reset_width, 44.0],
                            egui::Button::new("Reiniciar")
                                .fill(Color32::TRANSPARENT)
                                .corner_radius(10),
                        )
                        .on_hover_text("Interrompe a sessão atual sem contabilizar um pomodoro")
                        .clicked()
                    {
                        self.reset();
                    }
                });
            });
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Uma sessão de cada vez. A próxima começa com você.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        });
    }

    fn tasks_ui(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 10.0;
        card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(468.0);
            let pending = self
                .state
                .tasks
                .iter()
                .filter(|t| t.completed_at.is_none())
                .count();
            let completed = self.state.tasks.len() - pending;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Minhas tarefas").size(20.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    badge(ui, &format!("{pending} pendentes"), MUTED);
                });
            });
            ui.label(
                RichText::new("Escolha um próximo passo para dar sua atenção.")
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(12.0);
            ui.add_enabled_ui(!self.dirty, |ui| {
                let mut add = false;
                egui::Frame::new()
                    .fill(INSET)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .corner_radius(10)
                    .inner_margin(7)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);
                                    add = ui
                                        .add_enabled(
                                            !self.new_task.trim().is_empty(),
                                            egui::Button::new(
                                                RichText::new("+").size(22.0).color(ACCENT),
                                            )
                                            .min_size(egui::vec2(34.0, 34.0))
                                            .fill(SELECTED)
                                            .stroke(egui::Stroke::NONE),
                                        )
                                        .on_hover_text("Adicionar tarefa")
                                        .clicked();
                                    let width = ui.available_width();
                                    let response = ui.add_sized(
                                        [width, 34.0],
                                        egui::TextEdit::singleline(&mut self.new_task)
                                            .desired_width(width)
                                            .hint_text("Adicionar uma tarefa...")
                                            .frame(false)
                                            .margin(egui::vec2(5.0, 8.0))
                                            .char_limit(240),
                                    );
                                    add |= response.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                },
                            );
                        });
                    });
                if add && self.state.add_task(&self.new_task) {
                    self.new_task.clear();
                    self.show_completed = false;
                    self.persist();
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (show_completed, label) in [
                        (false, format!("Pendentes  {pending}")),
                        (true, format!("Concluídas  {completed}")),
                    ] {
                        let selected = self.show_completed == show_completed;
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(label).size(12.0).color(if selected {
                                        ACCENT
                                    } else {
                                        MUTED
                                    }),
                                )
                                .fill(if selected {
                                    SELECTED
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.show_completed = show_completed;
                        }
                    }
                });
                ui.add_space(6.0);
                let tasks: Vec<_> = self
                    .state
                    .tasks
                    .iter()
                    .filter(|t| t.completed_at.is_some() == self.show_completed)
                    .cloned()
                    .collect();
                if tasks.is_empty() {
                    ui.add_space(44.0);
                    ui.vertical_centered(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 28.0, SELECTED);
                        let c = rect.center();
                        ui.painter().add(egui::Shape::line(
                            vec![
                                c + egui::vec2(-10.0, 0.0),
                                c + egui::vec2(-3.0, 7.0),
                                c + egui::vec2(11.0, -8.0),
                            ],
                            egui::Stroke::new(2.5_f32, ACCENT),
                        ));
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(if self.show_completed {
                                "Cada conclusão conta"
                            } else if completed > 0 {
                                "Tudo em dia"
                            } else {
                                "Comece com uma tarefa"
                            })
                            .size(17.0)
                            .strong(),
                        );
                        ui.label(
                            RichText::new(if self.show_completed {
                                "Suas tarefas concluídas vão aparecer aqui."
                            } else {
                                "Adicione algo pequeno e dê o primeiro passo."
                            })
                            .size(12.0)
                            .color(MUTED),
                        );
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("task-list")
                        .max_height(300.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for task in tasks {
                                self.task_row(ui, &task);
                            }
                        });
                }
                if self.timer.active() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Sessão ativa · reinicie para trocar a tarefa em foco.")
                            .size(11.0)
                            .color(MUTED),
                    );
                }
            });
        });
    }

    fn task_row(&mut self, ui: &mut egui::Ui, task: &Task) {
        ui.push_id(task.id, |ui| {
            let selected = self.state.selected_task == Some(task.id);
            let active = self.timer.active() && selected;
            let completed = task.completed_at.is_some();
            egui::Frame::new()
                .fill(if selected { SELECTED } else { INSET })
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    if selected {
                        ACCENT.gamma_multiply(0.55)
                    } else {
                        BORDER
                    },
                ))
                .corner_radius(11)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                    ui.spacing_mut().button_padding = egui::vec2(7.0, 4.0);
                    ui.spacing_mut().interact_size.y = 26.0;
                    ui.horizontal_top(|ui| {
                        let mut done = completed;
                        if ui
                            .add_enabled(!active, egui::Checkbox::without_text(&mut done))
                            .on_hover_text(if completed {
                                "Reabrir tarefa"
                            } else {
                                "Concluir tarefa"
                            })
                            .changed()
                        {
                            self.state.toggle_task(task.id, Utc::now());
                            self.persist();
                        }
                        let text_width = (ui.available_width() - 40.0).max(40.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(text_width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(text_width);
                                let text = RichText::new(&task.title).size(14.0);
                                ui.add(
                                    egui::Label::new(if completed {
                                        text.strikethrough().color(MUTED)
                                    } else {
                                        text
                                    })
                                    .truncate(),
                                )
                                .on_hover_text(&task.title);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} pomodoros",
                                            self.state.pomodoros_for_task(task.id)
                                        ))
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if !completed {
                                                if ui
                                                    .add_enabled(
                                                        !self.timer.active(),
                                                        egui::Button::new(
                                                            RichText::new(if selected {
                                                                "Liberar"
                                                            } else {
                                                                "Focar"
                                                            })
                                                            .size(11.0)
                                                            .color(if selected {
                                                                MUTED
                                                            } else {
                                                                ACCENT
                                                            }),
                                                        )
                                                        .frame(false),
                                                    )
                                                    .clicked()
                                                {
                                                    self.state.selected_task =
                                                        if selected { None } else { Some(task.id) };
                                                    self.persist();
                                                }
                                                if selected {
                                                    badge(ui, "Em foco", ACCENT);
                                                }
                                            }
                                        },
                                    );
                                });
                            },
                        );
                        ui.menu_button(RichText::new("···").size(17.0).color(MUTED), |ui| {
                            if ui.button("Editar tarefa").clicked() {
                                self.editing = Some((task.id, task.title.clone()));
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    !active,
                                    egui::Button::new(
                                        RichText::new("Excluir tarefa")
                                            .color(Color32::from_rgb(238, 161, 147)),
                                    ),
                                )
                                .clicked()
                            {
                                self.deleting = Some(task.id);
                                ui.close();
                            }
                        });
                    });
                    if let Some((id, title)) =
                        self.editing.as_mut().filter(|(id, _)| *id == task.id)
                    {
                        ui.add_space(6.0);
                        ui.add(
                            egui::TextEdit::singleline(title)
                                .char_limit(240)
                                .desired_width(f32::INFINITY),
                        );
                        let (id, title) = (*id, title.clone());
                        ui.horizontal(|ui| {
                            if ui.button("Salvar").clicked() && self.state.edit_task(id, &title) {
                                self.editing = None;
                                self.persist();
                            }
                            if ui.button("Cancelar").clicked() {
                                self.editing = None;
                            }
                        });
                    }
                });
            ui.add_space(2.0);
        });
    }

    fn progress_ui(&self, ui: &mut egui::Ui) {
        let (tasks, focus) = self.state.daily_progress(Local::now().date_naive());
        ui.columns(3, |columns| {
            for (index, value, title) in [(0, focus, "POMODOROS"), (1, tasks, "TAREFAS CONCLUÍDAS")]
            {
                egui::Frame::new()
                    .fill(SURFACE)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .corner_radius(12)
                    .inner_margin(16)
                    .show(&mut columns[index], |ui| {
                        ui.set_min_width(ui.available_width());
                        eyebrow(ui, title);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{value:02}")).size(27.0).color(TEXT));
                            ui.label(RichText::new("hoje").size(12.0).color(MUTED));
                        });
                    });
            }
            egui::Frame::new()
                .fill(SURFACE)
                .stroke(egui::Stroke::new(1.0_f32, BORDER))
                .corner_radius(12)
                .inner_margin(16)
                .show(&mut columns[2], |ui| {
                    ui.set_min_width(ui.available_width());
                    eyebrow(ui, "SEU CICLO");
                    ui.horizontal(|ui| {
                        for i in 0..4 {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(18.0, 34.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                rect.center(),
                                6.0,
                                if i < self.state.focus_in_cycle {
                                    ACCENT
                                } else {
                                    BORDER
                                },
                            );
                        }
                        ui.label(
                            RichText::new(format!("{} / 4", self.state.focus_in_cycle))
                                .size(12.0)
                                .color(MUTED),
                        )
                        .on_hover_text("Uma pausa longa a cada quatro pomodoros concluídos");
                    });
                });
        });
    }

    fn history_ui(&self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, BORDER))
            .corner_radius(12)
            .inner_margin(16)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.collapsing(
                    RichText::new("Histórico de sessões")
                        .size(13.0)
                        .color(MUTED),
                    |ui| {
                        ui.add_space(8.0);
                        if self.state.sessions.is_empty() {
                            ui.label(
                                RichText::new("Seu histórico começa com a primeira sessão.")
                                    .color(MUTED),
                            );
                        }
                        for session in self.state.sessions.iter().rev().take(30) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(
                                        session
                                            .started_at
                                            .with_timezone(&Local)
                                            .format("%d/%m  %H:%M")
                                            .to_string(),
                                    )
                                    .size(12.0)
                                    .color(MUTED),
                                );
                                ui.label(RichText::new(session.kind.label()).size(12.0));
                                let width = (ui.available_width() - 115.0).max(30.0);
                                ui.add_sized(
                                    [width, 28.0],
                                    egui::Label::new(
                                        RichText::new(
                                            session.task_title.as_deref().unwrap_or("Sem tarefa"),
                                        )
                                        .size(12.0)
                                        .color(MUTED),
                                    )
                                    .truncate(),
                                );
                                let (label, color) = match session.outcome {
                                    Outcome::Active => ("Em andamento", ACCENT),
                                    Outcome::Completed => ("Concluído", ACCENT),
                                    Outcome::Interrupted => ("Interrompido", MUTED),
                                };
                                badge(ui, label, color);
                            });
                        }
                    },
                );
            });
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Configurações")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(400.0)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("No seu ritmo").size(22.0).strong());
                ui.label(RichText::new("Ajuste o tempo para a sua rotina.").color(MUTED));
                ui.add_space(16.0);
                eyebrow(ui, "DURAÇÃO DAS SESSÕES");
                egui::Grid::new("settings-grid")
                    .spacing([32.0, 12.0])
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("Foco", &mut self.draft.focus_minutes),
                            ("Pausa curta", &mut self.draft.short_break_minutes),
                            ("Pausa longa", &mut self.draft.long_break_minutes),
                        ] {
                            ui.label(label);
                            ui.add(egui::DragValue::new(value).range(1..=180).suffix(" min"));
                            ui.end_row();
                        }
                    });
                ui.add_space(16.0);
                eyebrow(ui, "AO TERMINAR UMA SESSÃO");
                ui.checkbox(&mut self.draft.sound, "Aviso sonoro");
                ui.checkbox(&mut self.draft.notifications, "Notificação do sistema");
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Novas durações valem a partir da próxima sessão.")
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(8.0);
                if ui
                    .add_enabled(
                        !self.dirty,
                        egui::Button::new(
                            RichText::new("Salvar configurações")
                                .strong()
                                .color(BACKGROUND),
                        )
                        .fill(ACCENT)
                        .min_size(egui::vec2(ui.available_width(), 40.0)),
                    )
                    .clicked()
                {
                    match self.storage.save_settings(&self.draft) {
                        Ok(()) => {
                            self.settings = self.draft.clone();
                            if !self.timer.active() {
                                self.timer =
                                    Timer::new(self.settings.duration(self.state.next_kind));
                            }
                            self.notice = "Configurações salvas.".into();
                            self.show_settings = false;
                        }
                        Err(error) => {
                            self.error = Some(format!(
                                "Não foi possível salvar as configurações: {error:#}"
                            ))
                        }
                    }
                }
                if ui.button("Testar avisos").clicked() {
                    self.notice = "Teste de aviso do Focus.".into();
                    self.alerts.push(crate::alerts::notify(
                        self.notice.clone(),
                        self.draft.notifications,
                        self.draft.sound,
                        self.audio.clone(),
                    ));
                }
            });
        self.show_settings &= open;
    }

    fn delete_ui(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.deleting {
            egui::Window::new("Excluir tarefa?")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .default_width(340.0)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("O histórico dos pomodoros será preservado.");
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.dirty
                                    && !(self.timer.active()
                                        && self.state.selected_task == Some(id)),
                                egui::Button::new("Excluir"),
                            )
                            .clicked()
                        {
                            self.state.delete_task(id);
                            self.deleting = None;
                            self.persist();
                        }
                        if ui.button("Cancelar").clicked() {
                            self.deleting = None;
                        }
                    });
                });
        }
    }
}

impl eframe::App for FocusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Closing takes precedence over a deadline reached between UI frames.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.reset();
            if !self.persist() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            return;
        }
        self.advance(Instant::now());
        self.alerts.retain(|receiver| {
            while let Ok(message) = receiver.try_recv() {
                self.error = Some(message);
            }
            // Receivers are cheap and only retained while workers are alive.
            match receiver.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
                Ok(message) => {
                    self.error = Some(message);
                    true
                }
                _ => true,
            }
        });
        ctx.request_repaint_after(Duration::from_millis(if self.timer.running() {
            200
        } else {
            1000
        }));
        self.apply_pending_window_size(ctx);
        if self.compact {
            self.compact_ui(ctx);
            return;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BACKGROUND).inner_margin(24.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("focus").size(30.0).color(ACCENT));
                    ui.label(RichText::new("Menos pressa. Mais presença.").color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Configurações").clicked() {
                            self.draft = self.settings.clone();
                            self.show_settings = true;
                        }
                        if ui
                            .button("Modo compacto")
                            .on_hover_text("Acompanhar a sessão em uma janela menor")
                            .clicked()
                        {
                            self.set_compact_mode(ctx, true);
                        }
                    });
                });
                ui.separator();
                self.error_ui(ui);
                egui::ScrollArea::vertical()
                    .id_salt("main-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            eyebrow(ui, "SEU ESPAÇO DE FOCO");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(
                                            Local::now().format("%d / %m / %Y").to_string(),
                                        )
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                },
                            );
                        });
                        ui.add_space(6.0);
                        self.progress_ui(ui);
                        ui.add_space(14.0);
                        if ui.available_width() >= 800.0 {
                            let gap = 18.0;
                            let width = ui.available_width();
                            let left = (width - gap) * 0.45;
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.horizontal_top(|ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(left, 0.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| self.timer_ui(ui),
                                );
                                ui.allocate_ui_with_layout(
                                    egui::vec2(width - gap - left, 0.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| self.tasks_ui(ui),
                                );
                            });
                        } else {
                            self.timer_ui(ui);
                            ui.add_space(10.0);
                            self.tasks_ui(ui);
                        }
                        if !self.notice.is_empty() {
                            ui.add_space(8.0);
                            egui::Frame::new()
                                .fill(SELECTED)
                                .corner_radius(10)
                                .inner_margin(14)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(RichText::new(&self.notice).size(12.0).color(ACCENT));
                                });
                        }
                        ui.add_space(12.0);
                        self.history_ui(ui);
                        ui.add_space(4.0);
                    });
            });
        self.settings_ui(ctx);
        self.delete_ui(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.state.finish(Utc::now(), Outcome::Interrupted);
        if let Err(error) = self.storage.save_state(&self.state) {
            eprintln!("Não foi possível salvar ao sair: {error:#}");
        }
    }
}
