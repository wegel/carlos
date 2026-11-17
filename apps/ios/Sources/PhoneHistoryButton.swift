// Keep older-history progress and retry in the conversation, without hiding saved text.
import SwiftUI

struct PhoneHistoryButton: View {
    let history: PhoneHistoryModel
    let load: () -> Void

    var body: some View {
        VStack(spacing: 4) {
            if let error = history.error {
                Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
            }
            Button(action: load) {
                HStack(spacing: 8) {
                    if history.loading { ProgressView().controlSize(.small) }
                    Text(history.loading ? "Loading earlier messages…" : history.error == nil ? "Earlier messages" : "Try earlier messages again")
                        .font(PhoneTheme.mono(12))
                }.frame(maxWidth: .infinity, minHeight: 44)
            }.disabled(history.loading).accessibilityIdentifier("transcript.history")
        }.padding(.horizontal, 16).foregroundStyle(PhoneTheme.overlay2)
    }
}
