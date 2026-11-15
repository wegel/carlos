//! Fence private clipboard epochs and preserve the complete source snapshot selected on screen.

use std::sync::Arc;

use carlos_core::session::models::{MessageKind, Role};
use carlos_proto::bytes::ByteLength;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::Source;

use super::super::{
    Facts, Formatter, Reply, Request, SelectionSlice, SelectionTarget, Target, View,
};
use super::{barrier, cache, feed, push, reply, show};
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_entry_cache::{PaintedRows, RowFormat};
use crate::app::transcript_input::RenderInput;
use crate::app::view_models::RenderedLine;

#[test]
fn a_selection_uses_the_displayed_prefix_even_after_newer_bytes_are_admitted() {
    let (mut cache, token) = cache();
    let mut formatter = Formatter::new().expect("worker");
    let job = formatter
        .open(token.clone(), OutputRevision::new(3))
        .expect("job");
    show(&mut formatter, 0);
    let old = "\x1b[31mold red 🌍\n".repeat(20);
    push(&mut cache, &token, 0, &old, old.len());
    feed(&mut formatter, &cache, old.len());
    let Reply::Frame { prepared, .. } = reply(&mut formatter) else {
        panic!("displayed frame")
    };
    let addition = "newer source after the selected snapshot\n";
    push(
        &mut cache,
        &token,
        old.len(),
        addition,
        old.len() + addition.len(),
    );
    feed(&mut formatter, &cache, old.len() + addition.len());
    barrier(&formatter);
    let copy = formatter.begin_copy().expect("private copy epoch");
    formatter
        .request_view(selection_view(copy, &old, prepared.count))
        .expect("old layout");
    let Reply::Selected { copy: acknowledged } = reply(&mut formatter) else {
        panic!("selected prefix")
    };
    assert_eq!(acknowledged, copy);
    formatter
        .sender
        .send(Request::Digest(job))
        .expect("owned prefix diagnostic");
    let Reply::Digest { digest, .. } = reply(&mut formatter) else {
        panic!("prefix digest")
    };
    assert_eq!(digest, blake3::hash(old.as_bytes()));
    assert_eq!(
        formatter.offset().expect("admitted counter").get(),
        (old.len() + addition.len()) as u64
    );
    formatter.cancel_copy();
    formatter.retire();
}

fn selection_view(copy: u64, text: &str, count: usize) -> View {
    let facts = Facts::from_input(RenderInput {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: "",
        file_path: None,
        label: None,
        label_generated: false,
        failed: false,
    });
    View {
        facts,
        format: RowFormat {
            width: 28,
            compact: false,
            theme: TranscriptTheme::Gutter,
            hidden: false,
        },
        previous: None,
        first: 0,
        length: count,
        target: Target::Selection(SelectionTarget {
            slice: SelectionSlice {
                copy,
                bounds: [1, 0, 28, count - 1],
                base: 0,
                finish: false,
            },
            source: Source {
                revision: OutputRevision::new(3),
                bytes: ByteLength::new(text.len() as u64),
            },
            count,
        }),
    }
}

#[test]
fn cached_selection_rows_need_no_source_job_or_original_string() {
    let mut formatter = Formatter::new().expect("copy worker");
    let copy = formatter.begin_copy().expect("copy epoch");
    let row = RenderedLine {
        text: "cached 🌍".into(),
        role: Role::User,
        styled_segments: Vec::new(),
        cells: 9,
        separator: false,
        soft_wrap_to_next: false,
    };
    let rows = PaintedRows::whole(Arc::new(vec![row]));
    let slice = SelectionSlice {
        copy,
        bounds: [1, 100, 9, 100],
        base: 100,
        finish: false,
    };
    assert!(formatter
        .selection_rows(slice, rows, 0, 1)
        .expect("bounded row request"));
    let Reply::Selected { copy: acknowledged } = reply(&mut formatter) else {
        panic!("cached row acknowledgement")
    };
    assert_eq!(acknowledged, copy);
    assert!(formatter.offset().is_none());
    formatter.cancel_copy();
}

#[test]
fn a_shorter_replacement_rejects_an_old_selection_before_waiting_for_its_bytes() {
    let (mut cache, token) = cache();
    let mut formatter = Formatter::new().expect("worker");
    formatter
        .open(token.clone(), OutputRevision::new(3))
        .expect("current job");
    show(&mut formatter, 0);
    push(&mut cache, &token, 0, "short", 5);
    feed(&mut formatter, &cache, 5);
    let _ = reply(&mut formatter);
    let copy = formatter.begin_copy().expect("copy epoch");
    let mut view = selection_view(copy, "an older much longer original", 100);
    let Target::Selection(target) = &mut view.target else {
        unreachable!()
    };
    target.source.revision = OutputRevision::new(2);
    formatter.request_view(view).expect("old selected snapshot");
    let Reply::CopyFailed {
        copy: rejected,
        error,
    } = reply(&mut formatter)
    else {
        panic!("a revision mismatch must not wait for more bytes")
    };
    assert_eq!(rejected, copy);
    assert_eq!(error, super::super::TransferError::Replaced);
    formatter.cancel_copy();
    formatter.retire();
}
