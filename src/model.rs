use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum SessionKind {
    #[default]
    Focus,
    ShortBreak,
    LongBreak,
}

impl SessionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "Foco",
            Self::ShortBreak => "Pausa curta",
            Self::LongBreak => "Pausa longa",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sound: bool,
    pub notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            sound: true,
            notifications: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            [
                self.focus_minutes,
                self.short_break_minutes,
                self.long_break_minutes
            ]
            .iter()
            .all(|m| (1..=180).contains(m)),
            "As durações devem estar entre 1 e 180 minutos."
        );
        Ok(())
    }
    pub fn duration(&self, kind: SessionKind) -> Duration {
        Duration::from_secs(
            u64::from(match kind {
                SessionKind::Focus => self.focus_minutes,
                SessionKind::ShortBreak => self.short_break_minutes,
                SessionKind::LongBreak => self.long_break_minutes,
            }) * 60,
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Outcome {
    Active,
    Completed,
    Interrupted,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Session {
    pub kind: SessionKind,
    pub task_id: Option<u64>,
    pub task_title: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: u64,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCompletion {
    pub task_id: u64,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub tasks: Vec<Task>,
    pub sessions: Vec<Session>,
    pub task_completions: Vec<TaskCompletion>,
    pub selected_task: Option<u64>,
    pub next_kind: SessionKind,
    pub focus_in_cycle: u8,
    next_id: u64,
}

impl State {
    pub fn add_task(&mut self, title: &str) -> bool {
        let title = title.trim();
        if title.is_empty() {
            return false;
        }
        self.next_id += 1;
        self.tasks.push(Task {
            id: self.next_id,
            title: title.into(),
            completed_at: None,
        });
        true
    }

    pub fn edit_task(&mut self, id: u64, title: &str) -> bool {
        if title.trim().is_empty() {
            return false;
        }
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.title = title.trim().into();
            return true;
        }
        false
    }

    pub fn toggle_task(&mut self, id: u64, at: DateTime<Utc>) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if task.completed_at.is_some() {
                task.completed_at = None;
                self.task_completions.retain(|c| c.task_id != id);
            } else {
                task.completed_at = Some(at);
                self.task_completions
                    .push(TaskCompletion { task_id: id, at });
                if self.selected_task == Some(id) {
                    self.selected_task = None;
                }
            }
        }
    }

    pub fn delete_task(&mut self, id: u64) {
        self.tasks.retain(|t| t.id != id);
        if self.selected_task == Some(id) {
            self.selected_task = None;
        }
    }

    pub fn begin(&mut self, at: DateTime<Utc>, settings: &Settings) {
        let task = if self.next_kind == SessionKind::Focus {
            self.tasks
                .iter()
                .find(|t| Some(t.id) == self.selected_task && t.completed_at.is_none())
        } else {
            None
        };
        self.sessions.push(Session {
            kind: self.next_kind,
            task_id: task.map(|t| t.id),
            task_title: task.map(|t| t.title.clone()),
            started_at: at,
            ended_at: None,
            duration_seconds: settings.duration(self.next_kind).as_secs(),
            outcome: Outcome::Active,
        });
    }

    pub fn finish(&mut self, at: DateTime<Utc>, outcome: Outcome) -> Option<SessionKind> {
        let session = self
            .sessions
            .iter_mut()
            .rev()
            .find(|s| s.outcome == Outcome::Active)?;
        session.outcome = outcome;
        session.ended_at = Some(at);
        let kind = session.kind;
        if outcome == Outcome::Completed {
            self.next_kind = match kind {
                SessionKind::Focus => {
                    self.focus_in_cycle = (self.focus_in_cycle + 1) % 4;
                    if self.focus_in_cycle == 0 {
                        SessionKind::LongBreak
                    } else {
                        SessionKind::ShortBreak
                    }
                }
                _ => SessionKind::Focus,
            };
        }
        Some(kind)
    }

    /// After an unclean exit, never infer a completion from wall-clock time.
    pub fn recover(&mut self, at: DateTime<Utc>) -> bool {
        let mut changed = false;
        for session in &mut self.sessions {
            if session.outcome == Outcome::Active {
                session.outcome = Outcome::Interrupted;
                session.ended_at = Some(at);
                changed = true;
            }
        }
        changed
    }

    pub fn pomodoros_for_task(&self, id: u64) -> usize {
        self.sessions
            .iter()
            .filter(|s| {
                s.task_id == Some(id)
                    && s.kind == SessionKind::Focus
                    && s.outcome == Outcome::Completed
            })
            .count()
    }

    pub fn daily_progress(&self, date: NaiveDate) -> (usize, usize) {
        let same_day = |at: DateTime<Utc>| at.with_timezone(&Local).date_naive() == date;
        let tasks = self
            .task_completions
            .iter()
            .filter(|c| same_day(c.at))
            .count();
        let focus = self
            .sessions
            .iter()
            .filter(|s| {
                s.kind == SessionKind::Focus
                    && s.outcome == Outcome::Completed
                    && s.ended_at.is_some_and(same_day)
            })
            .count();
        (tasks, focus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_uses_long_break_after_four_focus_sessions() {
        let mut state = State::default();
        let settings = Settings::default();
        let now = Utc::now();
        for n in 1..=4 {
            state.begin(now, &settings);
            state.finish(now, Outcome::Completed);
            assert_eq!(
                state.next_kind,
                if n == 4 {
                    SessionKind::LongBreak
                } else {
                    SessionKind::ShortBreak
                }
            );
            state.begin(now, &settings);
            state.finish(now, Outcome::Completed);
            assert_eq!(state.next_kind, SessionKind::Focus);
        }
        assert_eq!(state.daily_progress(Local::now().date_naive()).1, 4);
    }

    #[test]
    fn interrupted_focus_never_counts_or_advances_cycle() {
        let mut state = State::default();
        state.add_task("Escrever");
        state.selected_task = Some(1);
        state.begin(Utc::now(), &Settings::default());
        state.finish(Utc::now(), Outcome::Interrupted);
        assert_eq!(state.pomodoros_for_task(1), 0);
        assert_eq!(state.next_kind, SessionKind::Focus);
        assert_eq!(state.focus_in_cycle, 0);
        assert!(state.finish(Utc::now(), Outcome::Completed).is_none());
    }

    #[test]
    fn recovery_and_deleted_tasks_preserve_history() {
        let mut state = State::default();
        let now = Utc::now();
        state.add_task("Ler");
        state.selected_task = Some(1);
        state.begin(now, &Settings::default());
        state.finish(now, Outcome::Completed);
        state.toggle_task(1, now);
        state.delete_task(1);
        assert_eq!(
            state.daily_progress(now.with_timezone(&Local).date_naive()),
            (1, 1)
        );
        assert_eq!(state.sessions[0].task_title.as_deref(), Some("Ler"));
        state.begin(now, &Settings::default());
        assert!(state.recover(now));
        assert!(!state.recover(now));
        assert_eq!(state.sessions[1].outcome, Outcome::Interrupted);
        assert_eq!(
            state
                .daily_progress(now.with_timezone(&Local).date_naive())
                .1,
            1
        );
    }

    #[test]
    fn task_validation_and_reopening() {
        let mut state = State::default();
        assert!(!state.add_task("  "));
        assert!(state.add_task(" A "));
        assert!(!state.edit_task(1, ""));
        assert!(state.edit_task(1, " B "));
        assert_eq!(state.tasks[0].title, "B");
        state.toggle_task(1, Utc::now());
        state.toggle_task(1, Utc::now());
        assert!(state.task_completions.is_empty());
    }
}
