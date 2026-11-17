//! Build plain exported rows and paint them through the production cache without a session owner.

use carlos_core::perf_fixture::{PerfData, PerfRow};
use carlos_core::session::entry_id::EntryId;
use carlos_core::session::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};
use carlos_proto::bytes::ByteLength;
use carlos_proto::output::OutputRevision;
use carlos_proto::session::{
    Entry, EntryFormat, EntryKind, LabelSource, Tool, ToolKind, ToolStatus,
};
use carlos_proto::source::Source;
use uuid::Uuid;

use crate::app::perf_view::PerfView;
use crate::app::render_entry_cache::RenderKey;
use crate::app::selection::{selected_text, Selection};
use crate::app::view_models::RenderedLine;

pub(in crate::app) fn row(role: Role, text: impl Into<String>) -> PerfRow {
    draft(EntryDraft {
        role,
        kind: MessageKind::Plain,
        text: text.into(),
        file_path: None,
        tool_facts: None,
        tool_output: None,
    })
}

pub(in crate::app) fn draft(message: EntryDraft) -> PerfRow {
    let revision = OutputRevision::new(1);
    let source = Source {
        revision,
        bytes: ByteLength::new(message.text.len() as u64),
    };
    let kind = match message.role {
        Role::User => EntryKind::User,
        Role::Reasoning => EntryKind::Reasoning,
        Role::Commentary => EntryKind::Commentary,
        Role::Assistant => EntryKind::Answer,
        Role::ToolCall => EntryKind::ToolCall,
        Role::ToolOutput => EntryKind::Tool,
        Role::System => EntryKind::Notice,
    };
    let tool = message.tool_output.map(|details| tool(details, revision));
    PerfRow {
        entry: Entry {
            id: format!("{}-0000000000000000", Uuid::new_v4().simple())
                .parse()
                .expect("fixture row"),
            kind,
            format: match message.kind {
                MessageKind::Plain => EntryFormat::Plain,
                MessageKind::UserSteer => EntryFormat::UserSteer,
                MessageKind::Diff => EntryFormat::Diff,
            },
            text: String::new(),
            file_path: message.file_path,
            source: Some(source.clone()),
            done: true,
            tool,
            operation: None,
        },
        source: Some((source, message.text)),
    }
}

fn tool(details: ToolOutputDetails, revision: OutputRevision) -> Tool {
    Tool {
        call: None,
        kind: ToolKind::Other,
        label: details.summary,
        label_source: if details.generated {
            LabelSource::Ai
        } else {
            LabelSource::Literal
        },
        status: if details.failed {
            ToolStatus::Failed
        } else {
            ToolStatus::Ok
        },
        summary: None,
        added: None,
        removed: None,
        has_output: true,
        output_revision: revision,
    }
}

pub(in crate::app) fn view(rows: Vec<PerfRow>) -> PerfView {
    PerfView::new(PerfData {
        relevant_items: rows.len(),
        rows,
    })
}

pub(in crate::app) fn append(view: &mut PerfView, role: Role, text: impl Into<String>) -> usize {
    let index = view.rows.len();
    view.rows.push(row(role, text));
    dirty(view, index);
    index
}

pub(in crate::app) fn replace(view: &mut PerfView, index: usize, text: impl Into<String>) {
    let text = text.into();
    let row = &mut view.rows[index];
    let revision = OutputRevision::new(
        row.source
            .as_ref()
            .expect("fixture source")
            .0
            .revision
            .get()
            + 1,
    );
    let source = Source {
        revision,
        bytes: ByteLength::new(text.len() as u64),
    };
    row.entry.source = Some(source.clone());
    if let Some(tool) = row.entry.tool.as_mut() {
        tool.output_revision = revision;
    }
    row.source = Some((source, text));
    dirty(view, index);
}

fn dirty(view: &mut PerfView, index: usize) {
    view.view.render_cache.invalidate_entry(
        index,
        &RenderKey::Entry(EntryId::from_wire(&view.rows[index].entry.id)),
    );
    view.view
        .render_cache
        .mark_transcript_dirty_from(view.rows.len(), index);
}

pub(in crate::app) fn snapshot(view: &mut PerfView, width: usize) -> Vec<RenderedLine> {
    view.ensure_rendered_lines(width);
    snapshot_current(view)
}

pub(in crate::app) fn snapshot_current(view: &mut PerfView) -> Vec<RenderedLine> {
    let count = view.rendered_line_count();
    if count > 0 {
        view.view
            .render_cache
            .materialize_inputs(view.rows.as_slice(), 0, count - 1);
    }
    (0..count)
        .map(|index| {
            view.view
                .render_cache
                .rendered_line_at(index)
                .expect("painted fixture row")
                .clone()
        })
        .collect()
}

pub(in crate::app) fn selected(view: &mut PerfView, selection: Selection) -> String {
    selected_text(selection, &snapshot_current(view))
}
