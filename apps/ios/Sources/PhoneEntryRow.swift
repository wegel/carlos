// Render stable entry roles in either phone layout, keeping complete source separate from compact row facts.
import SwiftUI

struct PhoneEntryRow: View {
    let host: PhoneHostModel
    let row: PhoneEntry
    var compact = false
    @Environment(PhonePreferences.self) private var preferences
    @State private var viewer = UUID()
    @State private var visible = false
    @State private var source: PhoneSourceModel?
    @State private var acceptedInline = ""
    @State private var inlineMarkup = PhoneMarkdownModel()

    private var color: Color { row.kind.textColor }
    private var accent: Color { row.kind.accent }

    var body: some View {
        Group {
            if let tool = row.tool { PhoneToolView(host: host, row: row, tool: tool) }
            else if row.kind == .notice { notice }
            else if row.kind == .user { PhoneUserMessage { contents } }
            else if compact { reasoningLine }
            else {
                HStack(alignment: .top, spacing: 8) {
                    if preferences.layout == .gutter {
                        VStack(spacing: 4) {
                            Text(row.kind.glyph).font(PhoneTheme.mono(13)).bold()
                            if row.kind != .answer { Rectangle().fill(color.opacity(0.55)).frame(width: 2) }
                        }.frame(width: 14).foregroundStyle(accent)
                    }
                    VStack(alignment: .leading, spacing: 8) {
                        if preferences.layout == .flow { PhoneEntryHeading(kind: row.kind) }
                        contents
                        if row.kind == .answer && row.done { copyButton }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .onAppear { visible = true; updateSource() }
        .onChange(of: inlineMarkup.document.map(ObjectIdentifier.init)) { _, _ in host.contentChanged() }
        .onChange(of: source?.document.map(ObjectIdentifier.init)) { _, _ in host.contentChanged() }
        .onChange(of: row.text.sourceBytes) { _, _ in updateSource() }
        .onChange(of: row.text.sourceRevision) { _, _ in updateSource() }
        .onChange(of: row.done) { _, _ in updateSource() }
        .onChange(of: row.text.preview) { _, value in if row.text.sourceBytes == nil { acceptedInline = value; if row.kind == .answer || row.kind == .commentary { inlineMarkup.setInline(value) } } }
        .onDisappear { visible = false; source?.pause(viewer: viewer) }
    }

    private var reasoningLine: some View {
        HStack(spacing: 10) {
            Text("◆").font(PhoneTheme.mono(11)).frame(width: 14)
                .modifier(ReasoningPulse(active: !row.done))
            Text(row.text.preview.isEmpty ? "Thinking…" : row.text.preview)
                .font(PhoneTheme.prose(13.5).italic()).lineLimit(1)
                .textSelection(.enabled)
        }.foregroundStyle(PhoneTheme.mauve).frame(minHeight: 26)
    }

    @ViewBuilder private var contents: some View {
        if let document = source?.document ?? inlineMarkup.document {
            PhoneMarkdownView(document: document, color: color, role: row.kind, changed: host.contentChanged)
        }
        else {
            Text(acceptedInline.isEmpty ? row.text.preview : acceptedInline)
                .font(row.kind == .reasoning ? PhoneTheme.prose().italic() : PhoneTheme.prose(15, weight: row.kind == .user ? .semibold : .regular))
                .lineSpacing(PhoneTheme.proseLeading).textSelection(.enabled).foregroundStyle(color)
        }
        if let source, source.document == nil && source.error == nil {
            ProgressView("Loading original text").font(PhoneTheme.mono(11.5))
        }
        if let error = source?.error { Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red) }
    }

    @ViewBuilder private var copyButton: some View {
        HStack {
            Spacer()
            if row.text.sourceBytes == nil { PhoneCopyButton(source: .inline(row.text.preview)) }
            else if let source, source.done, let file = source.file {
                PhoneCopyButton(source: .original(file, source.acceptedBytes))
            }
        }
    }

    private var notice: some View {
        HStack(spacing: 8) {
            Rectangle().fill(PhoneTheme.surface0).frame(height: 1)
            if let document = source?.document {
                PhoneMarkdownView(document: document, color: color, role: .notice, changed: host.contentChanged)
            } else {
                Text(row.text.preview).font(PhoneTheme.mono(11.5)).foregroundStyle(color).multilineTextAlignment(.center)
            }
            Rectangle().fill(PhoneTheme.surface0).frame(height: 1)
        }.frame(minHeight: 28)
    }

    private func updateSource() {
        guard visible, row.tool == nil else { return }
        if row.text.sourceBytes == nil { acceptedInline = row.text.preview; if row.kind == .answer || row.kind == .commentary { inlineMarkup.setInline(row.text.preview) } }
        else {
            if source == nil { source = host.source(for: row.id) }
            source?.activate(viewer: viewer, revision: row.text.sourceRevision, bytes: row.text.sourceBytes, complete: row.done)
        }
    }

}

private struct ReasoningPulse: ViewModifier {
    let active: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dimmed = false

    func body(content: Content) -> some View {
        content.opacity(active && !reduceMotion && dimmed ? 0.4 : 1)
            .animation(active && !reduceMotion ? .easeInOut(duration: 0.55).repeatForever(autoreverses: true) : nil, value: dimmed)
            .onAppear { dimmed = active && !reduceMotion }
            .onChange(of: active) { _, value in dimmed = value && !reduceMotion }
            .onChange(of: reduceMotion) { _, value in dimmed = active && !value }
    }
}
