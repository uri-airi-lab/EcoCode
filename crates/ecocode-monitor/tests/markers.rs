use ecocode_monitor::markers::{MarkerState, parse_line};
use serde_json::json;

fn apply_all(lines: &[&str]) -> MarkerState {
    let mut state = MarkerState::default();
    for line in lines {
        if let Some(event) = parse_line(line) {
            state.apply(&event);
        }
    }
    state
}

#[test]
fn every_sdk_event_updates_its_field() {
    let state = apply_all(&[
        r#"{"event":"phase","value":"forward","sent_at_ns":1}"#,
        r#"{"event":"tokens","count":4096}"#,
        r#"{"event":"step","value":12}"#,
        r#"{"event":"loss","value":1.25}"#,
        r#"{"event":"learning_rate","value":0.0003}"#,
        r#"{"event":"sequence_length","value":512}"#,
        r#"{"event":"accuracy","value":0.9}"#,
        r#"{"event":"custom","key":"grad_norm","value":0.75}"#,
    ]);
    assert_eq!(state.current_phase.as_deref(), Some("forward"));
    assert_eq!(state.tokens_processed, Some(4096));
    assert_eq!(state.training_step, Some(12));
    assert_eq!(state.loss, Some(1.25));
    assert_eq!(state.learning_rate, Some(0.0003));
    assert_eq!(state.sequence_length, Some(512));
    assert_eq!(state.accuracy, Some(0.9));
    assert_eq!(state.custom_metrics_opt().unwrap()["grad_norm"], 0.75);
}

#[test]
fn epoch_becomes_a_phase_label() {
    let state = apply_all(&[r#"{"event":"epoch","value":3}"#]);
    assert_eq!(state.current_phase.as_deref(), Some("epoch:3"));
}

#[test]
fn later_markers_overwrite_earlier_ones() {
    let state = apply_all(&[
        r#"{"event":"phase","value":"forward"}"#,
        r#"{"event":"phase","value":"backward"}"#,
    ]);
    assert_eq!(state.current_phase.as_deref(), Some("backward"));
}

#[test]
fn blank_invalid_and_unknown_lines_are_ignored() {
    let state = apply_all(&["", "   ", "not json", r#"{"event":"mystery","value":1}"#, r#"{"no_event":1}"#]);
    assert_eq!(state, MarkerState::default());
    assert_eq!(state.custom_metrics_opt(), None);
}

#[test]
fn wrong_value_types_clear_the_field() {
    let mut state = MarkerState::default();
    state.apply(&json!({"event": "loss", "value": 1.0}));
    state.apply(&json!({"event": "loss", "value": "nan-ish"}));
    assert_eq!(state.loss, None);
}

#[test]
fn custom_marker_without_numeric_value_is_dropped() {
    let state = apply_all(&[r#"{"event":"custom","key":"note","value":"text"}"#]);
    assert!(state.custom_metrics.is_empty());
}

#[test]
fn parse_line_trims_whitespace_and_crlf() {
    assert_eq!(parse_line("  {\"event\":\"step\",\"value\":1}\r\n"), Some(json!({"event":"step","value":1})));
}
