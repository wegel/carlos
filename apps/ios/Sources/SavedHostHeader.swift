// Make saved servers identifiable and put edit/remove actions beside each server.
import SwiftUI

struct SavedHostHeader: View {
    let host: PhoneHostModel
    let edit: () -> Void
    let remove: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(host.configuration.name).font(PhoneTheme.prose(14, weight: .semibold))
                Text(host.configuration.address).font(PhoneTheme.mono(11.5))
                    .foregroundStyle(PhoneTheme.overlay2).lineLimit(1).truncationMode(.middle)
                    .accessibilityIdentifier("host.address.\(host.id)")
            }
            Spacer(minLength: 0)
            Menu {
                Button("Edit connection", systemImage: "pencil", action: edit)
                    .accessibilityIdentifier("host.edit.\(host.id)")
                if !host.connected {
                    Button("Reconnect", systemImage: "arrow.clockwise") { host.connect() }
                }
                Button("Remove from iPhone", systemImage: "trash", role: .destructive, action: remove)
                    .accessibilityIdentifier("host.remove.\(host.id)")
            } label: {
                Label("Manage", systemImage: "ellipsis").font(PhoneTheme.prose(13))
                    .frame(minHeight: 44)
            }
            .accessibilityLabel("Manage \(host.configuration.name)")
            .accessibilityIdentifier("host.manage.\(host.id)")
        }
    }
}
