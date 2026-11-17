// Keep the fixed SwiftUI controls around a native reusable transcript list.
import SwiftUI

struct PhoneTranscriptView: View {
    let host: PhoneHostModel
    let openQuestions: (PhoneApproval, PhoneQuestions) -> Void
    @Environment(PhonePreferences.self) private var preferences
    @State private var showLatest = false

    var body: some View {
        GeometryReader { geometry in
            PhoneTranscriptCollection(host: host, preferences: preferences,
                items: PhoneTranscriptCellItem.read(host: host, width: geometry.size.width, preferences: preferences),
                version: host.contentVersion, followRequest: host.followRequest, session: host.selected,
                openQuestions: openQuestions, readingChanged: { value in if showLatest != value { showLatest = value } })
                .overlay(alignment: .bottom) {
                    if showLatest || host.transcript?.hasNewer == true {
                        Button {
                            host.history.latest(host: host)
                            host.followRequest = UUID()
                        } label: {
                            Label("Latest", systemImage: "arrow.down").font(PhoneTheme.mono(12))
                                .padding(.horizontal, 16).frame(minHeight: 44)
                                .background(PhoneTheme.surface0).clipShape(Capsule())
                        }.accessibilityIdentifier("transcript.latest").padding(.bottom, 12)
                    }
                }
        }
    }
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
