// Keep voice failures visible beside the draft, including after a native permission dialog closes.
import SwiftUI

struct PhoneSpeechNotice: View {
    let speech: PhoneDictation

    var body: some View {
        if let message = speech.message {
            HStack(alignment: .top, spacing: 8) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(message).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext1)
                        .accessibilityIdentifier("voice.error")
                    if speech.permissionDenied {
                        Button("Open Settings") {
                            if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                        }.font(PhoneTheme.prose(13, weight: .semibold)).frame(minHeight: 44)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(.top, 12)
                Button { speech.message = nil } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }
                    .accessibilityLabel("Dismiss voice message").accessibilityIdentifier("voice.dismiss")
            }.padding(.leading, 16).padding(.bottom, 8)
                .background(PhoneTheme.base)
        }
    }
}
