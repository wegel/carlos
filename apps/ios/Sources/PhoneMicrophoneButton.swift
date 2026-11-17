// Keep the microphone reachable and honor Reduce Motion while recording.
import SwiftUI

struct PhoneMicrophoneButton: View {
    let speech: PhoneDictation
    let start: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button { if speech.phase == .listening { speech.stop() } else { start() } } label: {
            Group {
                if speech.phase == .authorizing || speech.phase == .finishing { ProgressView().tint(PhoneTheme.text) }
                else {
                    Image(systemName: speech.active ? "stop.fill" : "mic.fill")
                        .font(.system(size: 18, weight: .semibold))
                        .symbolEffect(.pulse, options: .repeating, isActive: speech.active && !reduceMotion)
                }
            }
            .frame(width: 44, height: 44).background(speech.active ? PhoneTheme.red : PhoneTheme.surface0)
            .foregroundStyle(speech.active ? PhoneTheme.crust : PhoneTheme.subtext1).clipShape(Circle())
        }
        .disabled(speech.phase == .authorizing || speech.phase == .finishing)
        .accessibilityLabel(speech.active ? "Stop voice input" : "Voice input")
        .accessibilityIdentifier("composer.microphone")
    }
}
