use viden_gui::{TranscriptRow, TranscriptViewport};

fn row(index: usize) -> TranscriptRow {
    TranscriptRow {
        id: format!("row-{index}"),
        kind: "assistant".into(),
        content: format!("output-{index}"),
    }
}

#[test]
fn fifty_thousand_rows_remain_bounded_and_follow_the_latest_by_default() {
    let mut viewport = TranscriptViewport::with_capacity(240);
    for index in 0..50_000 {
        viewport.append(row(index));
    }

    assert_eq!(viewport.len(), 240);
    assert_eq!(viewport.rows().front().unwrap().id, "row-49760");
    assert_eq!(viewport.rows().back().unwrap().id, "row-49999");
    assert_eq!(viewport.anchor(), Some("row-49999"));
    assert_eq!(viewport.new_output_count(), 0);
}

#[test]
fn history_scroll_keeps_its_anchor_and_counts_new_output() {
    let mut viewport = TranscriptViewport::with_capacity(240);
    for index in 0..10_000 {
        viewport.append(row(index));
    }
    viewport.set_follow_latest(false, Some("row-9900".into()));

    for index in 10_000..10_037 {
        viewport.append(row(index));
    }

    assert!(!viewport.follow_latest());
    assert_eq!(viewport.anchor(), Some("row-9900"));
    assert_eq!(viewport.new_output_count(), 37);
    let anchored = viewport.visible_range(180, 36);
    assert!(
        viewport
            .rows()
            .range(anchored)
            .any(|row| row.id == "row-9900")
    );
    viewport.set_follow_latest(true, None);
    assert_eq!(viewport.anchor(), Some("row-10036"));
    assert_eq!(viewport.new_output_count(), 0);
}

#[test]
fn resize_and_idle_recalculate_only_the_visible_window() {
    let mut viewport = TranscriptViewport::with_capacity(240);
    for index in 0..500 {
        viewport.append(row(index));
    }

    let compact = viewport.visible_range(180, 36);
    let roomy = viewport.visible_range(720, 36);
    assert_eq!(compact.end - compact.start, 7);
    assert_eq!(roomy.end - roomy.start, 22);

    let before = viewport.clone();
    for _ in 0..10_000 {
        let _ = viewport.visible_range(720, 36);
    }
    assert_eq!(viewport, before, "idle layout reads must not mutate state");
}
