// Start with native lazy rows; the full entry renderer and measured scroll checks extend this view.
import SwiftUI

struct PhoneTranscriptView: View {
    let host: PhoneHostModel
    @State private var following = true
    @State private var userScrolling = false
    @State private var metrics = TranscriptMetrics()

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 18) {
                    if host.selected == nil {
                        Text("Choose a session").font(PhoneTheme.prose(15, weight: .semibold))
                            .frame(maxWidth: .infinity).padding(.vertical, 40)
                    } else if host.orderedEntries.isEmpty {
                        Text(host.transcript?.ready == true ? "New session" : "Loading session…")
                            .font(PhoneTheme.prose(15, weight: .semibold))
                            .frame(maxWidth: .infinity).padding(.vertical, 40)
                    }
                    ForEach(host.orderedEntries, id: \.self) { id in
                        Group {
                            if let row = host.rows[id] { entry(row) }
                            else { ProgressView().frame(height: 26).onAppear { host.loadRows([id]) } }
                        }.id(id).accessibilityIdentifier("entry.\(id)")
                    }
                    ForEach(privateEchoes, id: \.key) { operation in
                        Text(operation.text ?? "").font(PhoneTheme.prose(15, weight: .semibold))
                            .padding(12).frame(maxWidth: .infinity, alignment: .leading).background(PhoneTheme.userBand)
                    }
                    ForEach(host.approvals, id: \.id) { card in PhoneApprovalView(host: host, card: card) }
                    ForEach(recoveries, id: \.key) { operation in PhoneRecoveryView(host: host, operation: operation) }
                    Color.clear.frame(height: 1).id("transcript.bottom")
                }.padding(.horizontal, 16).padding(.top, 6).padding(.bottom, 20)
            }
            .scrollIndicators(.hidden).defaultScrollAnchor(.bottom)
            .onScrollGeometryChange(for: TranscriptMetrics.self) { geometry in
                TranscriptMetrics(height: geometry.contentSize.height,
                    viewport: geometry.containerSize.height,
                    distance: max(0, geometry.contentSize.height + geometry.contentInsets.bottom
                        - geometry.contentOffset.y - geometry.containerSize.height))
            } action: { old, new in
                metrics = new
                if userScrolling { following = new.distance <= 40 }
                if following && !userScrolling && (old.height != new.height || old.viewport != new.viewport) {
                    proxy.scrollTo("transcript.bottom", anchor: .bottom)
                }
            }
            .onScrollPhaseChange { _, phase in
                switch phase {
                case .tracking, .interacting, .decelerating: userScrolling = true
                case .idle:
                    if userScrolling { following = metrics.distance <= 40 }
                    userScrolling = false
                case .animating: break
                @unknown default: break
                }
            }
            .onChange(of: host.selected) { _, _ in
                following = true
                proxy.scrollTo("transcript.bottom", anchor: .bottom)
            }
            .onChange(of: host.followRequest) { _, _ in
                following = true
                proxy.scrollTo("transcript.bottom", anchor: .bottom)
            }
            .overlay(alignment: .bottom) {
                if !following && metrics.distance > 260 {
                    Button {
                        following = true
                        withAnimation { proxy.scrollTo("transcript.bottom", anchor: .bottom) }
                    } label: {
                        Label("Latest", systemImage: "arrow.down").font(PhoneTheme.mono(12))
                            .padding(.horizontal, 16).frame(minHeight: 44)
                            .background(PhoneTheme.surface0).clipShape(Capsule())
                    }.accessibilityIdentifier("transcript.latest").padding(.bottom, 12)
                }
            }
        }
        .accessibilityIdentifier("transcript")
    }

    private var privateEchoes: [PhoneOperation] {
        host.operationOrder.compactMap { host.operations[$0] }.filter { $0.session == host.selected && $0.needsEcho }
    }

    private var recoveries: [PhoneOperation] {
        host.operationOrder.compactMap { host.operations[$0] }.filter {
            guard $0.session == host.selected, $0.text != nil else { return false }
            switch $0.state { case .rejected, .uncertain: return true; default: return false }
        }
    }

    private func entry(_ row: PhoneEntry) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if let tool = row.tool {
                HStack {
                    Text(tool.label).font(PhoneTheme.mono(12.5)).lineLimit(tool.labelSource == .ai ? nil : 1)
                    Spacer()
                    if tool.status == .running { ProgressView().controlSize(.small) }
                    else if let summary = tool.summary { Text(summary).font(PhoneTheme.mono(11.5)) }
                }.frame(minHeight: 26)
            } else {
                Text(label(row.kind)).font(PhoneTheme.mono()).foregroundStyle(color(row.kind))
                Text(row.text.preview).font(PhoneTheme.prose(15, weight: row.kind == .user ? .semibold : .regular))
                    .textSelection(.enabled).foregroundStyle(color(row.kind))
                if row.text.sourceBytes != nil {
                    Text("Original source has not loaded yet").font(PhoneTheme.mono(11.5)).foregroundStyle(PhoneTheme.overlay2)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(row.kind == .user ? 12 : 0)
        .background(row.kind == .user ? PhoneTheme.userBand : .clear)
    }

    private func label(_ kind: PhoneEntryKind) -> String {
        switch kind {
        case .user: "❯ you"; case .reasoning: "◆ reasoning"; case .commentary: "• commentary"
        case .answer: "✓ answer"; case .notice: "notice"; case .toolCall, .tool: "tool"
        }
    }

    private func color(_ kind: PhoneEntryKind) -> Color {
        switch kind {
        case .user: PhoneTheme.blue; case .reasoning: PhoneTheme.mauve; case .commentary: PhoneTheme.flamingo
        case .answer: PhoneTheme.text; case .notice: PhoneTheme.overlay2; case .toolCall, .tool: PhoneTheme.subtext1
        }
    }
}

private struct TranscriptMetrics: Equatable {
    var height: CGFloat = 0
    var viewport: CGFloat = 0
    var distance: CGFloat = 0
}
