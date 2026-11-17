// Give long documents independent native rows instead of nesting two lazy lists with one enormous height.
import SwiftUI

struct PhoneTranscriptItem: Identifiable {
    enum Content {
        case entry(PhoneEntry?, PhoneEntryLayout)
        case heading(PhoneEntry)
        case block(PhoneEntry, PhoneSourceModel, PhoneRenderedDocument, UInt32)
        case footer(PhoneEntry, PhoneSourceModel)
        case echo(PhoneOperation)
    }

    let id: String
    let entry: String
    let gap: CGFloat
    let content: Content

    @MainActor
    static func read(host: PhoneHostModel, width: CGFloat, layout: PhoneLayout, tools: PhoneToolMode) -> [Self] {
        var result: [Self] = []
        let echoes = PhoneEchoLayout(host: host)
        result += echoes.after(nil)
        var previous: PhoneEntryLayout?
        for id in host.orderedEntries {
            let row = host.rows[id]
            if tools != .full && row?.tool?.hasResult == true {
                result += echoes.after(id)
                continue
            }
            let form = row.map { PhoneEntryLayout.presentation($0, width: width) } ?? .block
            guard form != .hidden else { result += echoes.after(id); continue }
            let gap: CGFloat = previous == nil ? 6 : previous == .compact && form == .compact ? 0 : 18
            previous = form
            if let row, row.kind == .answer || row.kind == .commentary,
               let source = host.loadedSource(for: id), let document = source.document {
                if layout == .flow {
                    result.append(Self(id: id, entry: id, gap: gap, content: .heading(row)))
                }
                for block in 0..<UInt32(document.blockCount()) {
                    result.append(Self(id: block == 0 && layout == .gutter ? id : id + "/block/\(block)", entry: id,
                        gap: block == 0 ? (layout == .flow ? 8 : gap) : PhoneTheme.paragraphGap,
                        content: .block(row, source, document, block)))
                }
                result.append(Self(id: id + "/footer", entry: id, gap: 2, content: .footer(row, source)))
            } else {
                result.append(Self(id: id, entry: id, gap: gap, content: .entry(row, form)))
            }
            let inserted = echoes.after(id)
            result += inserted
            if !inserted.isEmpty { previous = .block }
        }
        result += echoes.tail
        return result
    }
}

struct PhoneTranscriptItemView: View {
    let host: PhoneHostModel
    let item: PhoneTranscriptItem
    @Environment(PhonePreferences.self) private var preferences

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            switch item.content {
            case .echo(let operation):
                PhoneUserMessage {
                    Text(operation.text ?? "").font(PhoneTheme.prose(15, weight: .semibold))
                        .lineSpacing(PhoneTheme.proseLeading).foregroundStyle(PhoneTheme.text)
                }
            case .entry(let row, let layout):
                if let row { PhoneEntryRow(host: host, row: row, compact: layout == .compact) }
                else { ProgressView().frame(height: 26).onAppear { host.loadRows([item.entry]) } }
            case .heading(let row):
                PhoneEntryHeading(kind: row.kind)
            case .block(let row, let source, let document, let index):
                HStack(alignment: .top, spacing: 8) {
                    if preferences.layout == .gutter {
                        if index == 0 {
                            Text(row.kind == .answer ? "✓" : "•").font(PhoneTheme.mono(13)).bold()
                                .foregroundStyle(row.kind == .answer ? PhoneTheme.green : PhoneTheme.flamingo).frame(width: 14)
                        } else { Color.clear.frame(width: 14) }
                    }
                    PhoneMarkdownBlockView(document: document, index: index,
                        color: row.kind == .answer ? PhoneTheme.text : PhoneTheme.flamingo, role: row.kind,
                        streaming: row.kind == .answer && !row.done && host.connected && host.transcript?.currentSnapshot == true && host.transcript?.info.activity.canInterrupt == true
                            && index + 1 == document.blockCount(), changed: host.contentChanged)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }.modifier(SourceRowLifetime(source: source, row: row))
            case .footer(let row, let source):
                VStack(alignment: .trailing, spacing: 4) {
                    if let error = source.error { Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red) }
                    if row.kind == .answer, row.done, source.done, let file = source.file {
                        PhoneCopyButton(source: .original(file, source.acceptedBytes))
                    }
                }.frame(maxWidth: .infinity, alignment: .trailing)
                    .modifier(SourceRowLifetime(source: source, row: row))
            }
        }.padding(.top, item.gap)
    }
}

// A device-local scroll/layout hint, never an invented native entry or a shared history fact.
struct PhoneEchoAnchor {
    let entry: String?
    let queued: Bool
}

// Insert only private echoes; confirmed native rows keep their original order.
@MainActor
private struct PhoneEchoLayout {
    private var positions: [String: [PhoneOperation]] = [:]
    private(set) var tail: [PhoneTranscriptItem] = []

    init(host: PhoneHostModel) {
        let known = Set(host.orderedEntries)
        for key in host.operationOrder {
            guard let operation = host.operations[key], operation.session == host.selected, operation.needsEcho else { continue }
            switch operation.state {
            case .rejected, .uncertain: continue
            default: break
            }
            guard let anchor = host.echoAnchors[key], !anchor.queued else {
                tail.append(Self.item(operation))
                continue
            }
            let entry = anchor.entry.flatMap { known.contains($0) ? $0 : nil } ?? ""
            positions[entry, default: []].append(operation)
        }
    }

    func after(_ entry: String?) -> [PhoneTranscriptItem] { (positions[entry ?? ""] ?? []).map(Self.item) }

    private static func item(_ operation: PhoneOperation) -> PhoneTranscriptItem {
        PhoneTranscriptItem(id: "echo/" + operation.key, entry: "", gap: 18, content: .echo(operation))
    }
}

private struct SourceRowLifetime: ViewModifier {
    let source: PhoneSourceModel
    let row: PhoneEntry
    @State private var viewer = UUID()

    func body(content: Content) -> some View {
        content.onAppear(perform: activate)
            .onChange(of: row.text.sourceRevision) { _, _ in activate() }
            .onChange(of: row.text.sourceBytes) { _, _ in activate() }
            .onChange(of: row.done) { _, _ in activate() }
            .onDisappear { source.pause(viewer: viewer) }
    }

    private func activate() {
        source.activate(viewer: viewer, revision: row.text.sourceRevision, bytes: row.text.sourceBytes, complete: row.done)
    }
}
