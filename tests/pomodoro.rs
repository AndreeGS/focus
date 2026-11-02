use chrono::{Local, Utc};
use focus::{
    model::{Outcome, SessionKind, Settings, State},
    timer::Timer,
};
use std::time::{Duration, Instant};

#[test]
fn delayed_focus_counts_for_its_task_and_waits_for_manual_break() {
    let settings = Settings::default();
    let mut state = State::default();
    state.add_task("Preparar apresentação");
    state.selected_task = Some(state.tasks[0].id);
    let now = Instant::now();
    let at = Utc::now();
    let duration = settings.duration(SessionKind::Focus);
    let mut timer = Timer::new(duration);
    state.begin(at, &settings);
    timer.start(now);

    if timer.tick(now + duration + Duration::from_secs(15)) {
        state.finish(at, Outcome::Completed);
    }
    assert_eq!(state.pomodoros_for_task(state.tasks[0].id), 1);
    assert_eq!(
        state.daily_progress(at.with_timezone(&Local).date_naive()),
        (0, 1)
    );
    assert_eq!(state.next_kind, SessionKind::ShortBreak);
    assert_eq!(state.sessions.len(), 1);
    assert!(!timer.running());
    assert!(!timer.tick(now + duration + Duration::from_secs(60)));
}

#[test]
fn closing_a_paused_focus_keeps_task_and_records_only_interruption() {
    let settings = Settings::default();
    let mut state = State::default();
    state.add_task("Ler um capítulo");
    let id = state.tasks[0].id;
    state.selected_task = Some(id);
    let now = Instant::now();
    let mut timer = Timer::new(settings.duration(SessionKind::Focus));
    state.begin(Utc::now(), &settings);
    timer.start(now);
    timer.pause(now + Duration::from_secs(30));
    assert!(!timer.tick(now + Duration::from_secs(7200)));
    state.finish(Utc::now(), Outcome::Interrupted);

    let saved = serde_json::to_string(&state).unwrap();
    let mut restored: State = serde_json::from_str(&saved).unwrap();
    assert!(!restored.recover(Utc::now()));
    assert_eq!(restored.sessions[0].outcome, Outcome::Interrupted);
    assert_eq!(restored.pomodoros_for_task(id), 0);
    assert_eq!(restored.selected_task, Some(id));
    assert_eq!(restored.next_kind, SessionKind::Focus);
    assert_eq!(restored.focus_in_cycle, 0);
}
