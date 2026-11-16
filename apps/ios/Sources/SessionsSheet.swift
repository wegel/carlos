// Group live session choices by configured host and create only explicitly requested sessions.
import SwiftUI

struct SessionsSheet: View {
    let store: PhoneStore
    @Environment(\.dismiss) private var dismiss
    @State private var connection = false
    @State private var backend = PhoneBackend.codex
    @State private var cwd = ""
    @State private var starting = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    ForEach(store.hosts) { host in hostGroup(host) }
                    newSession
                    Button("Connect another host") { connection = true }
                        .frame(minHeight: 44).accessibilityIdentifier("sessions.addHost")
                }
                .padding(16)
            }
            .background(PhoneTheme.base).navigationTitle("Sessions")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .tint(PhoneTheme.blue).font(PhoneTheme.prose())
        .presentationDetents([.fraction(0.8)]).presentationDragIndicator(.visible)
        .presentationCornerRadius(22)
        .onAppear { cwd = store.current?.transcript?.info.cwd ?? "" }
        .onChange(of: store.current?.selected) { old, new in
            if starting, new != nil, new != old { dismiss() }
        }
        .sheet(isPresented: $connection) { ConnectionView(store: store) { connection = false } }
    }

    private func hostGroup(_ host: PhoneHostModel) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(host.configuration.name).font(PhoneTheme.mono(11.5))
                Spacer()
                if !host.connected {
                    Button("Reconnect") { host.connect() }.frame(minHeight: 44)
                }
            }
            .foregroundStyle(PhoneTheme.overlay2)
            ForEach(host.catalog, id: \.id) { session in
                Button {
                    store.selectedHost = host.id
                    host.select(session.id)
                    dismiss()
                } label: {
                    HStack(spacing: 10) {
                        Circle().fill(session.statusColor).frame(width: 7, height: 7)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(session.title).font(PhoneTheme.prose(15, weight: .medium)).lineLimit(1)
                            Text([session.backend.label, session.cwd].compactMap { $0 }.joined(separator: " · "))
                                .font(PhoneTheme.mono(11.5)).foregroundStyle(PhoneTheme.overlay2).lineLimit(1)
                        }
                        Spacer(minLength: 8)
                        Text(session.status).font(PhoneTheme.mono(11.5)).foregroundStyle(session.statusColor)
                    }
                    .padding(12).frame(minHeight: 58)
                    .background(store.selectedHost == host.id && host.selected == session.id ? PhoneTheme.surface0 : .clear)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }
                .buttonStyle(.plain).disabled(!host.connected)
                .accessibilityIdentifier("session.\(host.id).\(session.id)")
            }
        }
    }

    private var newSession: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("new session").font(PhoneTheme.mono(11.5)).foregroundStyle(PhoneTheme.overlay2)
            Picker("Host", selection: Binding(get: { store.selectedHost ?? "" }, set: { store.selectedHost = $0 })) {
                ForEach(store.hosts) { host in Text(host.configuration.name).tag(host.id) }
            }
            .disabled(store.current?.creating == true)
            TextField("Working directory on the host", text: $cwd)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .font(PhoneTheme.mono()).frame(minHeight: 44).accessibilityIdentifier("sessions.cwd")
            Picker("Backend", selection: $backend) {
                Text("codex").tag(PhoneBackend.codex)
                Text("claude").tag(PhoneBackend.claude)
                Text("grok").tag(PhoneBackend.grok)
            }.pickerStyle(.segmented).accessibilityIdentifier("sessions.backend")
            Button {
                starting = true
                store.current?.create(backend: backend, cwd: cwd)
            } label: {
                if store.current?.creating == true { ProgressView() }
                else { Text("Start") }
            }
            .frame(maxWidth: .infinity, minHeight: 44).background(PhoneTheme.blue)
            .foregroundStyle(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 12))
            .disabled(store.current?.connected != true || store.current?.creating == true || cwd.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityIdentifier("sessions.start")
            if let error = store.current?.error { Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red) }
        }
    }
}
