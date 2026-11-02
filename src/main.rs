mod alerts;
mod ui;

use focus::storage::Storage;

fn main() -> anyhow::Result<()> {
    let storage = Storage::open()?;
    let (settings, mut state) = storage.load()?;
    if state.recover(chrono::Utc::now()) {
        storage.save_state(&state)?;
    }
    storage.save_settings(&settings)?;
    let audio = storage.prepare_sound().ok();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Focus · Pomodoro e tarefas")
            .with_inner_size(ui::FULL_WINDOW_SIZE)
            .with_min_inner_size(ui::FULL_MIN_SIZE),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Focus",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::FocusApp::new(
                cc, storage, settings, state, audio,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("Não foi possível abrir a interface: {error}"))
}
