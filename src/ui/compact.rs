use super::*;

const COMPACT_SIZE: egui::Vec2 = egui::vec2(360.0, 330.0);
const COMPACT_MIN_SIZE: egui::Vec2 = egui::vec2(320.0, 330.0);

impl FocusApp {
    pub(super) fn set_compact_mode(&mut self, ctx: &egui::Context, compact: bool) {
        if self.compact == compact {
            return;
        }
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        self.pending_window_size = None;
        if compact {
            self.full_window_maximized = maximized;
            if !self.full_window_maximized {
                // ViewportInfo::inner_rect is unavailable on Wayland. The egui
                // viewport rectangle still provides the content size there.
                self.full_window_size = ctx.viewport_rect().size().max(FULL_MIN_SIZE);
            }
            self.show_settings = false;
            self.deleting = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            // Relax the original minimum before asking the OS to shrink.
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(COMPACT_MIN_SIZE));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(COMPACT_SIZE));
            if maximized {
                self.pending_window_size = Some(COMPACT_SIZE);
            }
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(FULL_MIN_SIZE));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.full_window_size));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.full_window_maximized));
            if maximized && !self.full_window_maximized {
                self.pending_window_size = Some(self.full_window_size);
            }
        }
        self.compact = compact;
        ctx.request_repaint();
    }

    pub(super) fn apply_pending_window_size(&mut self, ctx: &egui::Context) {
        // Wayland ignores a size request while unmaximizing is still pending.
        // Retry once the compositor has confirmed the window is unmaximized.
        if ctx.input(|i| i.viewport().maximized == Some(false))
            && let Some(size) = self.pending_window_size.take()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
    }

    pub(super) fn compact_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(18))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("focus").size(24.0).color(ACCENT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Expandir")
                            .on_hover_text("Voltar à tela completa")
                            .clicked()
                        {
                            self.set_compact_mode(ctx, false);
                        }
                    });
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("compact-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.error_ui(ui);
                        ui.horizontal(|ui| {
                            badge(ui, self.state.next_kind.label(), ACCENT);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let status = if self.timer.running() {
                                        "Em andamento"
                                    } else if self.timer.active() {
                                        "Pausado"
                                    } else {
                                        "Pronto para começar"
                                    };
                                    ui.label(RichText::new(status).size(11.0).color(MUTED));
                                },
                            );
                        });
                        ui.add_space(6.0);
                        let remaining = self.timer.remaining(Instant::now());
                        let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(format!("{:02}:{:02}", seconds / 60, seconds % 60))
                                    .size(62.0)
                                    .color(TEXT),
                            );
                        });
                        let progress = (1.0
                            - remaining.as_secs_f32() / self.timer.duration().as_secs_f32())
                        .clamp(0.0, 1.0);
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .fill(ACCENT)
                                .desired_height(4.0),
                        );
                        ui.add_space(8.0);
                        let focus = self.state.next_kind == SessionKind::Focus;
                        eyebrow(
                            ui,
                            if focus {
                                "TAREFA DA SESSÃO"
                            } else {
                                "FAÇA UMA PAUSA"
                            },
                        );
                        let task = self
                            .state
                            .tasks
                            .iter()
                            .find(|t| Some(t.id) == self.state.selected_task);
                        let title = if focus {
                            task.map_or("Foco livre", |t| t.title.as_str())
                        } else {
                            "Respire, alongue-se e descanse."
                        };
                        ui.add(
                            egui::Label::new(RichText::new(title).size(14.0).color(ACCENT))
                                .truncate(),
                        )
                        .on_hover_text(title);
                        ui.add_space(10.0);
                        ui.add_enabled_ui(!self.dirty, |ui| {
                            let gap = ui.spacing().item_spacing.x;
                            let primary_width = (ui.available_width() - gap) * 0.63;
                            let reset_width = ui.available_width() - gap - primary_width;
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        [primary_width, 40.0],
                                        egui::Button::new(
                                            RichText::new(self.timer_action_label())
                                                .strong()
                                                .color(BACKGROUND),
                                        )
                                        .fill(ACCENT)
                                        .stroke(egui::Stroke::NONE),
                                    )
                                    .clicked()
                                {
                                    self.toggle_timer();
                                }
                                if ui
                                    .add_sized(
                                        [reset_width, 40.0],
                                        egui::Button::new("Reiniciar").fill(Color32::TRANSPARENT),
                                    )
                                    .on_hover_text(
                                        "Interrompe a sessão atual sem contabilizar um pomodoro",
                                    )
                                    .clicked()
                                {
                                    self.reset();
                                }
                            });
                        });
                        if !self.notice.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new(&self.notice).size(11.0).color(ACCENT));
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture {
        app: FocusApp,
        ctx: egui::Context,
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            static ID: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "focus-compact-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
            ));
            let storage = Storage::at(root.join("config"), root.join("data")).unwrap();
            let ctx = egui::Context::default();
            let cc = eframe::CreationContext::_new_kittest(ctx.clone());
            let mut state = State::default();
            state.add_task("Uma tarefa com um título longo para acompanhar a sessão de foco");
            state.selected_task = Some(1);
            let settings = Settings {
                sound: false,
                notifications: false,
                ..Settings::default()
            };
            Self {
                app: FocusApp::new(&cc, storage, settings, state, None),
                ctx,
                root,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn input(size: egui::Vec2) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        }
    }

    #[test]
    fn mode_switch_keeps_paused_session_and_restores_window_size() {
        let mut fixture = Fixture::new();
        let app = &mut fixture.app;
        let now = Instant::now();
        app.state.begin(Utc::now(), &app.settings);
        app.timer.start(now);
        app.timer.pause(now + Duration::from_secs(12));
        let state = serde_json::to_value(&app.state).unwrap();
        let remaining = app.timer.remaining(now);
        let full_size = egui::vec2(1180.0, 900.0);
        let output = fixture.ctx.run(input(full_size), |ctx| {
            app.set_compact_mode(ctx, true);
            assert!(app.compact);
            app.set_compact_mode(ctx, false);
        });
        assert!(!app.compact);
        assert!(app.timer.active());
        assert!(!app.timer.running());
        assert_eq!(
            app.timer.remaining(now + Duration::from_secs(200)),
            remaining
        );
        assert_eq!(serde_json::to_value(&app.state).unwrap(), state);
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.iter().any(
            |cmd| matches!(cmd, egui::ViewportCommand::InnerSize(size) if *size == COMPACT_SIZE)
        ));
        assert!(commands.iter().any(
            |cmd| matches!(cmd, egui::ViewportCommand::InnerSize(size) if *size == full_size)
        ));
    }

    #[test]
    fn mode_switch_keeps_running_timer_and_restores_maximized_state() {
        let mut fixture = Fixture::new();
        let app = &mut fixture.app;
        let now = Instant::now();
        app.state.begin(Utc::now(), &app.settings);
        app.timer.start(now);
        let before = app.timer.remaining(now + Duration::from_secs(10));
        let mut input = input(FULL_WINDOW_SIZE);
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(true);
        let output = fixture.ctx.run(input, |ctx| {
            app.set_compact_mode(ctx, true);
            app.set_compact_mode(ctx, false);
        });
        assert!(app.timer.running());
        assert_eq!(app.timer.remaining(now + Duration::from_secs(10)), before);
        assert_eq!(app.state.sessions.len(), 1);
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, egui::ViewportCommand::Maximized(true)))
        );
    }

    #[test]
    fn compact_resize_waits_for_unmaximize_confirmation() {
        let mut fixture = Fixture::new();
        let mut maximized = input(FULL_WINDOW_SIZE);
        maximized
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(true);
        let _ = fixture.ctx.run(maximized.clone(), |ctx| {
            fixture.app.set_compact_mode(ctx, true)
        });
        assert_eq!(fixture.app.pending_window_size, Some(COMPACT_SIZE));
        let _ = fixture.ctx.run(maximized.clone(), |ctx| {
            fixture.app.apply_pending_window_size(ctx)
        });
        assert_eq!(fixture.app.pending_window_size, Some(COMPACT_SIZE));
        maximized
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(false);
        let output = fixture
            .ctx
            .run(maximized, |ctx| fixture.app.apply_pending_window_size(ctx));
        assert!(fixture.app.pending_window_size.is_none());
        assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|cmd| matches!(cmd, egui::ViewportCommand::InnerSize(size) if *size == COMPACT_SIZE)));
    }

    #[test]
    fn compact_controls_fit_in_minimum_window() {
        let mut fixture = Fixture::new();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, COMPACT_MIN_SIZE);
        let mut output = None;
        for _ in 0..3 {
            output = Some(
                fixture
                    .ctx
                    .run(input(COMPACT_MIN_SIZE), |ctx| fixture.app.compact_ui(ctx)),
            );
        }
        fn text_bounds(shape: &egui::Shape, text: &str) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Text(t) if t.galley.job.text == text => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
                }
                egui::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|shape| text_bounds(shape, text))
                }
                _ => None,
            }
        }
        let output = output.unwrap();
        for text in ["Expandir", "Iniciar sessão", "Reiniciar", "25:00"] {
            let rect = output
                .shapes
                .iter()
                .find_map(|shape| text_bounds(&shape.shape, text))
                .unwrap_or_else(|| panic!("Controle ausente: {text}"));
            assert!(
                screen.contains_rect(rect),
                "Controle fora da janela: {text}, {rect:?}"
            );
        }
    }
}
