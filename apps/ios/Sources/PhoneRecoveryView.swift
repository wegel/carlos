// Keep failed private text accessible without replacing a newer draft or resending it automatically.
import SwiftUI

struct PhoneRecoveryView: View {
    let host: PhoneHostModel
    let operation: PhoneOperation

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Submitted text retained").font(PhoneTheme.prose(13, weight: .semibold))
            Text(operation.text ?? "").font(PhoneTheme.prose()).textSelection(.enabled)
            HStack {
                Button { host.restoreDraft(operation.key) } label: {
                    Text("Restore to empty draft").font(PhoneTheme.prose(13, weight: .medium))
                        .frame(minHeight: 44).padding(.horizontal, 10).contentShape(Rectangle())
                }
                .foregroundStyle(host.draft.isEmpty ? PhoneTheme.blue : PhoneTheme.overlay1)
                .background(PhoneTheme.surface0).clipShape(RoundedRectangle(cornerRadius: 10))
                .disabled(!host.draft.isEmpty).accessibilityIdentifier("recovery.restore.\(operation.key)")
                Spacer(minLength: 8)
                PhoneCopyButton(source: .inline(operation.text ?? ""), identifier: "recovery.copy.\(operation.key)")
                Button(role: .destructive) { host.discardDraft(operation.key) } label: {
                    Text("Discard").font(PhoneTheme.prose(13, weight: .medium))
                        .frame(minHeight: 44).padding(.horizontal, 10).contentShape(Rectangle())
                }
                .foregroundStyle(PhoneTheme.red)
                .accessibilityLabel("Discard retained text")
                .accessibilityIdentifier("recovery.discard.\(operation.key)")
            }
            Text("Check the outcome before sending it again.").font(PhoneTheme.prose(13))
                .foregroundStyle(PhoneTheme.overlay2)
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .background(PhoneTheme.base).clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
