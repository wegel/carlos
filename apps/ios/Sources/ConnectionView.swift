// Accept explicit host credentials; unencrypted WebSocket requires the person's private-network choice.
import SwiftUI

struct ConnectionView: View {
    let store: PhoneStore
    let existing: PhoneHost?
    var completed: () -> Void = {}
    @Environment(\.dismiss) private var dismiss
    @State private var address = ""
    @State private var host = ""
    @State private var token = ""
    @State private var privateWebSocket = false
    @State private var error: String?

    init(store: PhoneStore, existing: PhoneHost? = nil, completed: @escaping () -> Void = {}) {
        self.store = store
        self.existing = existing
        self.completed = completed
        _address = State(initialValue: existing?.address ?? "")
        _host = State(initialValue: existing?.name ?? "")
        _privateWebSocket = State(initialValue: existing?.allowsPrivateWebSocket ?? false)
    }

    private var usesSavedToken: Bool {
        existing?.address == address.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Carlos daemon") {
                    TextField("Host name", text: $host).accessibilityIdentifier("connection.host")
                    TextField("wss://host:port", text: $address)
                        .textContentType(.URL).keyboardType(.URL)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityIdentifier("connection.address")
                    SecureField("Device token", text: $token)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityIdentifier("connection.token")
                    if usesSavedToken {
                        Text("Leave the token blank to keep using the saved token.")
                            .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.overlay2)
                    }
                    Toggle("Allow unencrypted WebSocket on a private network", isOn: $privateWebSocket)
                        .accessibilityIdentifier("connection.private")
                }
                Section {
                    Button("Connect", action: connect)
                        .disabled(address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
                                  (token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !usesSavedToken))
                        .frame(minHeight: 44).accessibilityIdentifier("connection.connect")
                    Text("Use a device token provisioned by your Carlos daemon. Carlos stores it in this device’s Keychain.")
                        .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.overlay2)
                    if let message = error ?? store.error {
                        Text(message).foregroundStyle(PhoneTheme.red)
                            .accessibilityIdentifier("connection.error")
                    }
                }
            }
            .scrollContentBackground(.hidden).background(PhoneTheme.mantle)
            .navigationTitle("Connect to Carlos")
            .toolbar {
                if !store.hosts.isEmpty {
                    ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                }
            }
            .font(PhoneTheme.prose()).tint(PhoneTheme.blue)
        }
    }

    private func connect() {
        do {
            try store.connect(name: host, address: address, token: token, privateWebSocket: privateWebSocket)
            token = ""
            completed()
        } catch { self.error = error.localizedDescription }
    }
}
