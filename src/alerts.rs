use std::{
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver},
    thread,
};

/// Desktop integrations run outside the UI thread. No shell interpolation.
pub fn notify(
    title: String,
    notifications: bool,
    sound: bool,
    audio: Option<PathBuf>,
) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    if notifications {
        let tx = tx.clone();
        thread::spawn(move || {
            let result = Command::new("notify-send")
                .args([
                    "--app-name=Focus",
                    "--icon=alarm-symbolic",
                    "--expire-time=10000",
                    "--",
                    "Focus",
                    &title,
                ])
                .output();
            if !result.is_ok_and(|o| o.status.success()) {
                let _ = tx.send("Não foi possível enviar a notificação. Verifique notify-send e o serviço de notificações.".into());
            }
        });
    }
    if sound {
        thread::spawn(move || {
            if let Some(path) = audio {
                for player in ["pw-play", "paplay", "aplay"] {
                    if Command::new(player)
                        .arg(&path)
                        .output()
                        .is_ok_and(|o| o.status.success())
                    {
                        return;
                    }
                }
            }
            let _ = tx.send("Não foi possível reproduzir o som. Verifique pw-play, paplay ou aplay e a saída de áudio.".into());
        });
    }
    rx
}
