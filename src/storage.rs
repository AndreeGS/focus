use crate::model::{Settings, State};
use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

pub struct Storage {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    _lock: File,
}

fn base_path(value: Option<PathBuf>, home: &Path, fallback: &str) -> PathBuf {
    value
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(fallback))
}

impl Storage {
    pub fn open() -> Result<Self> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .context("HOME precisa apontar para um diretório absoluto.")?;
        Self::at(
            base_path(
                env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
                &home,
                ".config",
            )
            .join("focus"),
            base_path(
                env::var_os("XDG_DATA_HOME").map(PathBuf::from),
                &home,
                ".local/share",
            )
            .join("focus"),
        )
    }

    pub fn at(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self> {
        for dir in [&config_dir, &data_dir] {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder
                .create(dir)
                .with_context(|| format!("Não foi possível criar {}", dir.display()))?;
        }
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        options.mode(0o600);
        let lock = options.open(data_dir.join("focus.lock"))?;
        lock.try_lock()
            .context("Não foi possível bloquear os dados. O Focus já pode estar aberto.")?;
        Ok(Self {
            config_dir,
            data_dir,
            _lock: lock,
        })
    }

    pub fn load(&self) -> Result<(Settings, State)> {
        let settings: Settings = read_json(&self.config_dir.join("settings.json"))?;
        settings.validate()?;
        let state: State = read_json(&self.data_dir.join("state.json"))?;
        anyhow::ensure!(
            state.focus_in_cycle < 4,
            "Ciclo inválido no arquivo de dados."
        );
        Ok((settings, state))
    }

    pub fn save_state(&self, state: &State) -> Result<()> {
        atomic_write(
            &self.data_dir.join("state.json"),
            &serde_json::to_vec_pretty(state)?,
        )
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        write_json(&self.config_dir.join("settings.json"), settings)
    }

    pub fn prepare_sound(&self) -> Result<PathBuf> {
        let path = self.data_dir.join("alarm.wav");
        atomic_write(&path, include_bytes!("../assets/alarm.wav"))?;
        Ok(path)
    }
}

fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "Arquivo inválido: {}. Preserve uma cópia antes de corrigir o conteúdo.",
                path.display()
            )
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(T::default()),
        Err(error) => {
            Err(error).with_context(|| format!("Não foi possível ler {}", path.display()))
        }
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("tmp");
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .with_context(|| format!("Não foi possível gravar {}", temporary.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(path.parent().context("Diretório ausente")?)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_recovery() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::at(root.path().join("config"), root.path().join("data")).unwrap();
        let (settings, mut state) = storage.load().unwrap();
        state.add_task("Minha tarefa");
        state.selected_task = Some(1);
        state.begin(chrono::Utc::now(), &settings);
        storage.save_settings(&settings).unwrap();
        storage.save_state(&state).unwrap();
        let (_, mut restored) = storage.load().unwrap();
        assert_eq!(restored.tasks[0].title, "Minha tarefa");
        assert!(restored.recover(chrono::Utc::now()));
        assert_eq!(restored.pomodoros_for_task(1), 0);
    }

    #[test]
    fn corrupt_data_is_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::at(root.path().join("config"), root.path().join("data")).unwrap();
        let path = storage.data_dir.join("state.json");
        fs::write(&path, "not json").unwrap();
        assert!(storage.load().is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "not json");
    }

    #[test]
    fn only_one_instance_can_write() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::at(root.path().join("config"), root.path().join("data")).unwrap();
        assert!(Storage::at(storage.config_dir.clone(), storage.data_dir.clone()).is_err());
        let config = storage.config_dir.clone();
        let data = storage.data_dir.clone();
        drop(storage);
        assert!(Storage::at(config, data).is_ok());
    }

    #[test]
    fn relative_xdg_paths_are_ignored() {
        assert_eq!(
            base_path(Some("relative".into()), Path::new("/home/test"), ".config"),
            PathBuf::from("/home/test/.config")
        );
        assert_eq!(
            base_path(Some("/custom".into()), Path::new("/home/test"), ".config"),
            PathBuf::from("/custom")
        );
    }
}
