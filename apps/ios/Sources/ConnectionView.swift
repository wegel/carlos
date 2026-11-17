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
    @State private var pending: PhoneHostModel?
    @FocusState private var field: Field?

    private enum Field: Hashable { case host, address, token }

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
                fields
                Section {
                    Text("Use a device token provisioned by your Carlos daemon. Carlos stores it in this device’s Keychain.")
                        .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.overlay2)
                }.listRowBackground(PhoneTheme.base)
            }
            .safeAreaInset(edge: .bottom, spacing: 0) { actionBar }
            .scrollDismissesKeyboard(.interactively)
            .scrollContentBackground(.hidden).background(PhoneTheme.mantle)
            .navigationTitle(existing == nil ? "Connect to Carlos" : "Connection")
            .navigationBarTitleDisplayMode(existing == nil ? .large : .inline)
            .toolbar {
                if existing != nil || store.current != nil {
                    ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                }
                if field != nil {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { field = nil }.accessibilityIdentifier("connection.keyboardDone")
                    }
                }
            }
            .font(PhoneTheme.prose()).tint(PhoneTheme.blue)
            .foregroundStyle(PhoneTheme.text)
            .onChange(of: pending?.connected) { _, connected in
                guard connected == true, let host = pending, host.error == nil else { return }
                store.selectedHost = host.id
                pending = nil
                token = ""
                completed()
            }
            .onChange(of: pending?.error) { _, message in
                guard let message else { return }
                error = message
                pending = nil
            }
        }
    }

    private var fields: some View {
        Section("Carlos daemon") {
            TextField("Host name", text: $host).accessibilityIdentifier("connection.host")
                .focused($field, equals: .host).submitLabel(.next).onSubmit { field = .address }
            TextField("wss://host:port", text: $address)
                .textContentType(.URL).keyboardType(.URL)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .accessibilityIdentifier("connection.address")
                .focused($field, equals: .address).submitLabel(.next).onSubmit { field = .token }
            SecureField(usesSavedToken ? "Saved token (unchanged)" : "Device token", text: $token)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .accessibilityIdentifier("connection.token")
                .focused($field, equals: .token).submitLabel(.go).onSubmit { if canConnect { connect() } }
            if usesSavedToken {
                Text("Leave the token blank to keep using the saved token.")
                    .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.overlay2)
            }
            Toggle("Allow unencrypted WebSocket on a private network", isOn: $privateWebSocket)
                .accessibilityIdentifier("connection.private")
        }.disabled(pending != nil).listRowBackground(PhoneTheme.base)
    }

    private var actionBar: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let message = error ?? store.error {
                Text(message).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                    .accessibilityIdentifier("connection.error")
            }
            Button(action: connect) {
                HStack(spacing: 8) {
                    if pending != nil { ProgressView().controlSize(.small).tint(PhoneTheme.crust) }
                    Text(pending == nil ? "Connect" : "Connecting…").font(PhoneTheme.prose(15, weight: .semibold))
                }.frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.plain).disabled(!canConnect || pending != nil)
            .background(canConnect ? PhoneTheme.blue : PhoneTheme.surface0)
            .foregroundStyle(canConnect ? PhoneTheme.crust : PhoneTheme.overlay1)
            .clipShape(RoundedRectangle(cornerRadius: 12)).accessibilityIdentifier("connection.connect")
        }.padding(16).background(PhoneTheme.mantle)
    }

    private var canConnect: Bool {
        !address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        (!token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || usesSavedToken)
    }

    private func connect() {
        do {
            error = nil
            field = nil
            pending = try store.connect(name: host, address: address, token: token, privateWebSocket: privateWebSocket)
        } catch { self.error = PhoneFailureMessage.describe(error) }
    }
}
