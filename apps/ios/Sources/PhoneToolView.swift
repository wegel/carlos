// Expand completed or running tools with the same checked output reader and device-local row choice.
import SwiftUI

struct PhoneToolView: View {
    let host: PhoneHostModel
    let row: PhoneEntry
    let tool: PhoneTool
    @Environment(PhonePreferences.self) private var preferences
    @Environment(\.preserveTranscriptPosition) private var preservePosition
    @State private var output: PhoneOutputModel?
    @State private var visible = false
    @State private var retainedHeight: CGFloat = 26

    private var expanded: Bool {
        tool.hasOutput && preferences.isExpanded(host: host.id, session: host.selected ?? "", entry: row.id)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                guard let session = host.selected else { return }
                preservePosition()
                preferences.toggle(host: host.id, session: session, entry: row.id)
            } label: { header.contentShape(Rectangle()) }
                .buttonStyle(.plain).disabled(!tool.hasOutput)
                .accessibilityIdentifier("tool.\(row.id)")
                .accessibilityValue(expanded ? "Expanded" : "Collapsed")
            if expanded {
                Group {
                    if let output {
                        PhoneOutputView(model: output, filePath: row.format == .plain ? row.filePath : nil)
                            .frame(minHeight: output.window == nil ? retainedHeight : 0)
                            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { retainedHeight = $0 }
                    } else { Color.clear.frame(height: retainedHeight).accessibilityHidden(true) }
                }.padding(.leading, 24).padding(.top, 8)
            }
        }
        .onAppear { visible = true; updateOutput() }
        .onChange(of: expanded) { _, _ in updateOutput() }
        .onDisappear { visible = false; closeOutput() }
    }

    private var header: some View {
        HStack(spacing: 10) {
            Text(glyph).font(PhoneTheme.mono(13)).bold().foregroundStyle(color).frame(width: 14)
            Text(PhoneEntryLayout.toolLabel(tool))
                .font(tool.labelSource == .ai ? PhoneTheme.prose(13.5) : PhoneTheme.mono(12.5))
                .lineLimit(tool.labelSource == .ai ? nil : 1).foregroundStyle(PhoneTheme.subtext1)
                .lineSpacing(tool.labelSource == .ai ? PhoneTheme.toolLeading : 0)
                .padding(.vertical, tool.labelSource == .ai ? 4 : 0)
                .frame(maxWidth: .infinity, alignment: .leading)
            Spacer(minLength: 4)
            metadata.lineLimit(1).fixedSize(horizontal: true, vertical: false)
            if tool.hasOutput {
                Image(systemName: "chevron.right").font(.system(size: 10)).foregroundStyle(PhoneTheme.overlay1)
                    .rotationEffect(.degrees(expanded ? 90 : 0))
            }
        }.font(PhoneTheme.mono(11.5)).frame(minHeight: 26)
    }

    private var metadata: some View {
        HStack(spacing: 6) {
            if tool.status == .running { ProgressView().controlSize(.mini).tint(PhoneTheme.mauve) }
            else if tool.status == .failed { Text("failed").bold().foregroundStyle(PhoneTheme.red) }
            else if tool.status == .interrupted { Text("interrupted").foregroundStyle(PhoneTheme.yellow) }
            else if let diff = tool.diff {
                if let added = diff.added { Text("+\(added)").foregroundStyle(PhoneTheme.green) }
                if let removed = diff.removed { Text("−\(removed)").foregroundStyle(PhoneTheme.red) }
            } else if let summary = tool.summary { Text(summary).foregroundStyle(PhoneTheme.overlay2) }
        }
    }

    private func updateOutput() {
        guard visible, expanded else { closeOutput(); return }
        if output == nil { output = host.output(for: row.id) }
        output?.start()
    }

    private func closeOutput() {
        if let output { host.closeOutput(output) }
        output = nil
    }

    private var glyph: String {
        switch tool.kind { case .command, .other: "$"; case .read: "→"; case .search: "✱"; case .edit: "Δ" }
    }

    private var color: Color {
        switch tool.kind { case .command, .other: PhoneTheme.peach; case .read: PhoneTheme.blue; case .search: PhoneTheme.teal; case .edit: PhoneTheme.yellow }
    }
}
