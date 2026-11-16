// Keep failed private text accessible without replacing a newer draft or resending it automatically.
import SwiftUI
import UIKit

struct PhoneRecoveryView: View {
    let host: PhoneHostModel
    let operation: PhoneOperation

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Submitted text retained").font(PhoneTheme.prose(13, weight: .semibold))
            Text(operation.text ?? "").font(PhoneTheme.prose()).textSelection(.enabled)
            HStack {
                Button("Restore to empty draft") { host.draft = operation.text ?? "" }
                    .disabled(!host.draft.isEmpty).frame(minHeight: 44)
                Button("Copy") { UIPasteboard.general.string = operation.text }.frame(minHeight: 44)
            }
            Text("Check the outcome before sending it again.").font(PhoneTheme.prose(13))
                .foregroundStyle(PhoneTheme.overlay2)
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .background(PhoneTheme.base).clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
