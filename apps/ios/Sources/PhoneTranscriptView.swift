// Keep stable native rows and follow only real transcript changes while the reader stays at the bottom.
import SwiftUI

struct PhoneTranscriptView: View {
    let host: PhoneHostModel
    @Environment(PhonePreferences.self) private var preferences
    @State private var position = ScrollPosition(edge: .bottom)
    @State private var following = true
    @State private var userScrolling = false
    @State private var metrics = TranscriptMetrics()

    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if host.selected == nil {
                        Text("Choose a session").font(PhoneTheme.prose(15, weight: .semibold))
                            .frame(maxWidth: .infinity).padding(.vertical, 40)
                    } else if host.orderedEntries.isEmpty {
                        Text(host.transcript?.ready == true ? "New session" : "Loading session…")
                            .font(PhoneTheme.prose(15, weight: .semibold))
                            .frame(maxWidth: .infinity).padding(.vertical, 40)
                    }
                    ForEach(PhoneTranscriptItem.read(host: host, width: geometry.size.width - 32, layout: preferences.layout)) { item in
                        PhoneTranscriptItemView(host: host, item: item).id(item.id)
                    }
                    ForEach(privateEchoes, id: \.key) { operation in
                        PhoneUserMessage {
                            Text(operation.text ?? "").font(PhoneTheme.prose(15, weight: .semibold))
                                .lineSpacing(PhoneTheme.proseLeading).foregroundStyle(PhoneTheme.text)
                        }.padding(.top, 18)
                    }
                    ForEach(host.approvals, id: \.id) { card in PhoneApprovalView(host: host, card: card).padding(.top, 18) }
                    ForEach(recoveries, id: \.key) { operation in PhoneRecoveryView(host: host, operation: operation).padding(.top, 18) }
                    Color.clear.frame(height: 1).id("transcript.bottom")
                }.scrollTargetLayout().frame(width: max(0, geometry.size.width - 32), alignment: .leading)
                    .padding(.horizontal, 16).padding(.top, 6).padding(.bottom, 20)
            }
            .clipped().scrollDismissesKeyboard(.interactively).scrollIndicators(.hidden).defaultScrollAnchor(.bottom, for: .initialOffset)
            .defaultScrollAnchor(following ? .bottom : nil, for: .sizeChanges)
            .scrollPosition($position)
            .onScrollGeometryChange(for: TranscriptMetrics.self) { geometry in
                TranscriptMetrics(height: geometry.contentSize.height,
                    viewport: geometry.containerSize.height,
                    distance: max(0, geometry.contentSize.height + geometry.contentInsets.bottom
                        - geometry.contentOffset.y - geometry.containerSize.height))
            } action: { old, new in
                metrics = new
                if userScrolling { following = new.distance <= 40 }
            }
            .onScrollPhaseChange { _, phase in
                switch phase {
                case .interacting, .decelerating: userScrolling = true
                case .tracking: break
                case .idle:
                    if userScrolling { following = metrics.distance <= 40 }
                    userScrolling = false
                case .animating: break
                @unknown default: break
                }
            }
            .onChange(of: host.selected) { _, _ in
                following = true
                position.scrollTo(id: "transcript.bottom", anchor: .bottom)
            }
            .onChange(of: host.contentVersion) { _, _ in
                if following && !userScrolling { position.scrollTo(id: "transcript.bottom", anchor: .bottom) }
            }
            .onChange(of: host.followRequest) { _, _ in
                following = true
                position.scrollTo(id: "transcript.bottom", anchor: .bottom)
            }
            .overlay(alignment: .bottom) {
                if !following && metrics.distance > 260 {
                    Button {
                        following = true
                        withAnimation { position.scrollTo(id: "transcript.bottom", anchor: .bottom) }
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

}

private struct TranscriptMetrics: Equatable {
    var height: CGFloat = 0
    var viewport: CGFloat = 0
    var distance: CGFloat = 0
}
