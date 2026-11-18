// Share the existing keyboard Done row between send timing and voice input.
import SwiftUI
import UIKit

struct PhoneComposerTools: View {
    let host: PhoneHostModel
    let speech: PhoneDictation
    let startVoice: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Button {
                speech.cancel(keepText: true)
                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
            } label: {
                Text("Done").font(.system(size: 14, weight: .semibold)).frame(minWidth: 56, minHeight: 44)
            }
            .foregroundStyle(PhoneTheme.blue).accessibilityLabel("Hide keyboard")
            .accessibilityIdentifier("composer.keyboardDone")
            PhoneMicrophoneButton(speech: speech, start: startVoice)
                .disabled(host.selected == nil || host.submitting)
            Spacer(minLength: 0)
            if host.transcript?.info.activity.canInterrupt == true && !host.draft.isEmpty {
                Menu {
                    Picker("Send while running", selection: Binding(get: { host.sendMode }, set: { host.sendMode = $0 })) {
                        Text("Steer the current response").tag(PhoneSendMode.steer)
                        Text("Queue for the next turn").tag(PhoneSendMode.queue)
                    }
                } label: {
                    HStack(spacing: 4) {
                        Text(host.sendMode == .steer ? "Steer" : "Queue")
                        Image(systemName: "chevron.down").font(.system(size: 8, weight: .semibold))
                    }.font(PhoneTheme.mono(11)).frame(minWidth: 44, minHeight: 44)
                }
                .disabled(host.submitting).foregroundStyle(PhoneTheme.subtext1)
                .accessibilityLabel("Send mode").accessibilityValue(host.sendMode == .steer ? "Steer" : "Queue")
                .accessibilityIdentifier("composer.mode")
            }
        }
        .padding(.horizontal, 12).frame(maxWidth: .infinity).frame(height: 44).background(PhoneTheme.mantle).ignoresSafeArea()
    }
}
