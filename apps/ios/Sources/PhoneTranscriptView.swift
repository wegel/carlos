// Keep stable native rows and follow only real transcript changes while the reader stays at the bottom.
import SwiftUI

struct PhoneTranscriptView: View {
    let host: PhoneHostModel
    let openQuestions: (PhoneApproval, PhoneQuestions) -> Void
    @Environment(PhonePreferences.self) private var preferences
    @State private var offset = PhoneScrollOffset()
    @State private var position = ScrollPosition(idType: String.self)
    @State private var following = true
    @State private var userScrolling = false
    @State private var metrics = TranscriptMeasurement()
    @State private var showLatest = false

    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if host.transcript?.hasOlder == true {
                        PhoneHistoryButton(history: host.history, load: loadHistory)
                    }
                    if host.selected == nil {
                        Text("Choose a session").font(PhoneTheme.prose(15, weight: .semibold))
                            .frame(maxWidth: .infinity).padding(.vertical, 40)
                    } else if host.orderedEntries.isEmpty {
                        Text(host.transcript?.ready == true ? "New session" : "Loading session…")
                            .font(PhoneTheme.prose(15, weight: .semibold))
                            .frame(maxWidth: .infinity).padding(.vertical, 40)
                    }
                    ForEach(PhoneTranscriptItem.read(host: host, width: geometry.size.width - 32, layout: preferences.layout, tools: preferences.toolMode)) { item in
                        PhoneTranscriptItemView(host: host, item: item).padding(.horizontal, 16).id(item.id)
                            .background(PhoneScrollAttachment(offset: offset, row: item.id, gap: item.gap))
                    }
                    ForEach(host.approvals, id: \.id) { card in PhoneApprovalView(host: host, card: card, openQuestions: openQuestions).padding(.horizontal, 16).padding(.top, 18) }
                    ForEach(recoveries, id: \.key) { operation in PhoneRecoveryView(host: host, operation: operation).padding(.horizontal, 16).padding(.top, 18) }
                    Color.clear.frame(height: 1).id("transcript.bottom")
                }.scrollTargetLayout().frame(width: max(0, geometry.size.width), alignment: .leading)
                    .background(PhoneScrollAttachment(offset: offset))
                    .padding(.bottom, 20)
            }
            .accessibilityIdentifier("transcript")
            .clipped().scrollDismissesKeyboard(.interactively).scrollIndicators(.hidden).defaultScrollAnchor(.bottom, for: .initialOffset)
            .scrollPosition($position)
            .environment(\.preserveTranscriptPosition) { entry in
                following = false
                metrics.anchor = offset.frames[entry].map { TranscriptAnchor(id: entry, entry: entry, y: $0.minY) }
                metrics.restoring = metrics.anchor != nil
                position.isPositionedByUser = true
                offset.scroll(to: metrics.value.offset)
            }
            .onScrollGeometryChange(for: TranscriptMetrics.self) { geometry in
                TranscriptMetrics(height: geometry.contentSize.height,
                    offset: geometry.contentOffset.y,
                    distance: max(0, geometry.contentSize.height + geometry.contentInsets.bottom
                        - geometry.contentOffset.y - geometry.containerSize.height))
            } action: { old, new in
                metrics.value = new
                // Native row frames settle after SwiftUI reports the new content size.
                DispatchQueue.main.async { restoreAnchor() }
                showLatest = new.distance > 260
                if userScrolling { following = new.distance <= 40 }
                else if new.height < old.height && new.distance <= 40 { following = true }
            }
            .onScrollPhaseChange { _, phase in
                switch phase {
                case .tracking, .interacting, .decelerating:
                    position.isPositionedByUser = true
                    userScrolling = true
                    metrics.anchor = nil
                    metrics.restoring = false
                case .idle:
                    if userScrolling {
                        following = metrics.value.distance <= 40
                        if metrics.value.offset <= 20 && host.history.error == nil { loadHistory() }
                    }
                    userScrolling = false
                case .animating: break
                @unknown default: break
                }
            }
            .onChange(of: host.selected) { _, _ in
                metrics.anchor = nil
                metrics.restoring = false
                following = true
                position.scrollTo(id: "transcript.bottom", anchor: .bottom)
            }
            .onChange(of: host.orderedEntries.first) { old, new in
                if old != new, let anchor = metrics.anchor, host.orderedEntries.contains(anchor.entry) {
                    metrics.restoring = true
                    DispatchQueue.main.async { restoreAnchor() }
                }
            }
            .onChange(of: host.contentVersion) { _, _ in
                if following && !userScrolling { position.scrollTo(id: "transcript.bottom", anchor: .bottom) }
            }
            .onChange(of: host.followRequest) { _, _ in
                following = true
                metrics.anchor = nil
                metrics.restoring = false
                position.scrollTo(id: "transcript.bottom", anchor: .bottom)
            }
            .overlay(alignment: .bottom) {
                if !following && showLatest || host.transcript?.hasNewer == true {
                    Button {
                        following = true
                        metrics.anchor = nil
                        metrics.restoring = false
                        host.history.latest(host: host)
                        withAnimation { position.scrollTo(id: "transcript.bottom", anchor: .bottom) }
                    } label: {
                        Label("Latest", systemImage: "arrow.down").font(PhoneTheme.mono(12))
                            .padding(.horizontal, 16).frame(minHeight: 44)
                            .background(PhoneTheme.surface0).clipShape(Capsule())
                    }.accessibilityIdentifier("transcript.latest").padding(.bottom, 12)
                }
            }
        }
    }

    private func restoreAnchor() {
        guard let anchor = metrics.anchor, metrics.restoring, !userScrolling else { return }
        guard let frame = offset.frames[anchor.id] else {
            position.scrollTo(id: anchor.id, anchor: .top)
            return
        }
        if abs(frame.minY - anchor.y) > 0.5 {
            position.isPositionedByUser = true
            offset.scroll(to: offset.y + frame.minY - anchor.y)
        }
    }

    private func loadHistory() {
        guard !host.history.loading else { return }
        following = false
        metrics.restoring = false
        if let visible = offset.frames.filter({ $0.value.maxY > 0 }).min(by: { $0.value.minY < $1.value.minY }) {
            let entry = host.orderedEntries.first { visible.key == $0 || visible.key.hasPrefix($0 + "/") }
            if let entry { metrics.anchor = TranscriptAnchor(id: visible.key, entry: entry, y: visible.value.minY) }
        }
        metrics.restoring = metrics.anchor != nil
        host.history.load(host: host)
    }

    private var recoveries: [PhoneOperation] {
        host.operationOrder.compactMap { host.operations[$0] }.filter {
            guard $0.session == host.selected, $0.text != nil else { return false }
            switch $0.state { case .rejected, .uncertain: return true; default: return false }
        }
    }

}

// Raw geometry is a measurement, not view state. Publish only the visible thresholds.
@MainActor
private final class TranscriptMeasurement {
    var value = TranscriptMetrics()
    var anchor: TranscriptAnchor?
    var restoring = false
}

private struct TranscriptAnchor {
    let id: String
    let entry: String
    let y: CGFloat
}

private struct TranscriptMetrics: Equatable {
    var height: CGFloat = 0
    var offset: CGFloat = 0
    var distance: CGFloat = 0
}

private struct PreserveTranscriptPosition: EnvironmentKey {
    static let defaultValue: @MainActor (String) -> Void = { _ in }
}

extension EnvironmentValues {
    var preserveTranscriptPosition: @MainActor (String) -> Void {
        get { self[PreserveTranscriptPosition.self] }
        set { self[PreserveTranscriptPosition.self] = newValue }
    }
}
