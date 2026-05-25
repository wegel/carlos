// Tests for mobile-mouse decoding and perf-overlay key handling.

#[test]
fn consume_mobile_mouse_char_requires_prefix_to_activate() {
    let mut app = AppState::new("thread-1".to_string());
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '<'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '7'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '6'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, ';'),
        MobileMouseConsume::Consumed
    ));
}

#[test]
fn consume_mobile_mouse_char_accepts_csi_bracket_prefix() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 12;
    app.viewport.mobile_mouse_last_y = Some(40);

    for ch in ['[', '<', '6', '4', ';', '7', '6', ';', '4', '6', 'M'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert_eq!(app.viewport.scroll_top, 18);
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn mobile_mouse_key_candidate_accepts_alt_prefixed_csi_chars() {
    let app = AppState::new("thread-1".to_string());
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('['),
        KeyModifiers::ALT
    ));
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::ALT
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
}

#[test]
fn mobile_mouse_key_candidate_accepts_plain_coords_when_pending() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.mobile_plain_pending_coords = true;
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char(';'),
        KeyModifiers::empty()
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('a'),
        KeyModifiers::empty()
    ));
}

#[test]
fn consume_mobile_mouse_char_plain_pending_pair_applies_scroll() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.mobile_plain_pending_coords = true;
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(50);

    for ch in ['6', '6', ';', '5', '2'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }

    assert_eq!(app.viewport.scroll_top, 23);
    assert!(!app.viewport.mobile_plain_pending_coords);
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_plain_pending_repeated_pair_reuses_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(50);

    app.viewport.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }
    assert_eq!(app.viewport.scroll_top, 23);

    app.viewport.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }
    assert_eq!(app.viewport.scroll_top, 26);
}

#[test]
fn consume_mobile_mouse_char_plain_pending_new_gesture_keeps_prior_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(50);
    app.viewport.mobile_plain_last_direction = 1;
    app.viewport.mobile_plain_new_gesture = true;
    app.viewport.mobile_plain_pending_coords = true;

    for ch in ['6', '4', ';', '4', '7'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }

    assert_eq!(app.viewport.scroll_top, 23);
    assert_eq!(app.viewport.mobile_plain_last_direction, 1);
}

#[test]
fn parse_repeated_plain_mobile_pair_accepts_concatenated_repetition() {
    assert_eq!(
        parse_repeated_plain_mobile_pair("75;4375;4375;43"),
        Some((75, 43))
    );
    assert_eq!(
        parse_repeated_plain_mobile_pair("71;5371;5371;53"),
        Some((71, 53))
    );
    assert_eq!(parse_repeated_plain_mobile_pair("75;43"), None);
}

#[test]
fn consume_mobile_mouse_char_applies_scroll_on_terminator() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 10;
    app.viewport.mobile_mouse_last_y = Some(40);

    for ch in ['<', '6', '4', ';', '7', '6', ';', '4', '6'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, 'M'),
        MobileMouseConsume::Consumed
    ));
    assert_eq!(app.viewport.scroll_top, 16);
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn apply_mobile_mouse_scroll_uses_natural_touch_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut app, 44);
    assert_eq!(app.viewport.scroll_top, 24);

    apply_mobile_mouse_scroll(&mut app, 42);
    assert_eq!(app.viewport.scroll_top, 22);
}

#[test]
fn apply_mobile_mouse_scroll_honors_invert_toggle() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_inverted = true;
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut app, 44);
    assert_eq!(app.viewport.scroll_top, 16);

    apply_mobile_mouse_scroll(&mut app, 42);
    assert_eq!(app.viewport.scroll_top, 18);
}

#[test]
fn kitt_head_index_bounces_across_separator() {
    let seq: Vec<usize> = (0..9).map(|tick| kitt_head_index(5, tick)).collect();
    assert_eq!(seq, vec![0, 1, 2, 3, 4, 3, 2, 1, 0]);
    assert_eq!(kitt_head_index(1, 42), 0);
}

#[test]

fn duration_samples_tracks_percentiles_and_window() {
    let mut samples = DurationSamples::new(3);
    samples.push(Duration::from_micros(1_000));
    samples.push(Duration::from_micros(2_000));
    samples.push(Duration::from_micros(3_000));
    samples.push(Duration::from_micros(4_000));

    // Window size is 3, so first sample is dropped.
    assert_eq!(samples.values_us.len(), 3);
    assert_eq!(samples.percentile_us(0.50), Some(3_000));
    assert_eq!(samples.percentile_us(0.95), Some(4_000));
    assert_eq!(samples.avg_ms(), 2.50);
    assert_eq!(samples.max_ms(), 4.0);
}

#[test]
fn perf_metrics_overlay_lines_include_latency_rows() {
    let mut perf = PerfMetrics::new();
    perf.poll_wait.push(Duration::from_micros(500));
    perf.event_handle.push(Duration::from_micros(700));
    perf.draw.push(Duration::from_micros(900));
    perf.key_interval.push(Duration::from_micros(1_100));
    perf.repeat_interval.push(Duration::from_micros(1_300));
    perf.press_to_first_repeat
        .push(Duration::from_micros(1_500));
    perf.release_to_next_key.push(Duration::from_micros(1_700));

    let lines = perf.overlay_lines();
    assert!(lines.iter().any(|line| line.contains("poll wait")));
    assert!(lines.iter().any(|line| line.contains("event handle")));
    assert!(lines.iter().any(|line| line.contains("draw")));
    assert!(lines.iter().any(|line| line.contains("key interval")));
    assert!(lines.iter().any(|line| line.contains("repeat intvl")));
    assert!(lines.iter().any(|line| line.contains("press->repeat")));
    assert!(lines.iter().any(|line| line.contains("release->key")));
}

#[test]
fn perf_metrics_tracks_repeat_transition_buckets() {
    let mut perf = PerfMetrics::new();
    perf.mark_key_kind(KeyEventKind::Press);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Release);
    perf.mark_key_kind(KeyEventKind::Press);

    assert_eq!(perf.key_press_events, 2);
    assert_eq!(perf.key_repeat_events, 2);
    assert_eq!(perf.key_release_events, 1);
    assert_eq!(perf.press_to_first_repeat.values_us.len(), 1);
    assert_eq!(perf.repeat_interval.values_us.len(), 1);
    assert_eq!(perf.release_to_next_key.values_us.len(), 1);
}
