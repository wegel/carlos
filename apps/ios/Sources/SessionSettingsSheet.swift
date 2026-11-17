// Send Ralph changes to the daemon and store only display choices on the phone.
import SwiftUI

struct SessionSettingsSheet: View {
    @Bindable var preferences: PhonePreferences
    @Bindable var host: PhoneHostModel
    let connection: () -> Void
    @Environment(\.dismiss) private var dismiss

    private var info: PhoneSession? { host.transcript?.info }
    private var request: PhoneOperation? { host.ralphRequest.flatMap { $0.session == host.selected ? $0 : nil } }
    private var waiting: Bool {
        guard let request else { return false }
        switch request.state {
        case .pending, .queued: return true
        case .accepted: return host.desiredRalph != info?.activity.ralph
        case .rejected, .uncertain: return false
        }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Tool output") {
                    Picker("Tool output", selection: $preferences.toolMode) {
                        ForEach(PhoneToolMode.allCases) { Text($0.rawValue).tag($0) }
                    }.pickerStyle(.segmented).accessibilityIdentifier("settings.tools")
                }
                Section("Layout") {
                    Picker("Layout", selection: $preferences.layout) {
                        ForEach(PhoneLayout.allCases) { Text($0.rawValue).tag($0) }
                    }.pickerStyle(.segmented).accessibilityIdentifier("settings.layout")
                }
                Section {
                    Toggle("Ralph mode", isOn: Binding(
                        get: { info?.activity.ralph ?? false },
                        set: { host.setRalph($0) }
                    ))
                    .tint(PhoneTheme.pink).frame(minHeight: 44)
                    .disabled(!host.connected || host.transcript?.ready != true || host.controlSubmitting || waiting)
                    .accessibilityIdentifier("settings.ralph")
                    if waiting {
                        Text("Waiting for the engine to apply Ralph \(host.desiredRalph == true ? "on" : "off")")
                            .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext0)
                    }
                }
                Section {
                    Button("Connection") { dismiss(); connection() }.frame(minHeight: 44)
                        .accessibilityIdentifier("settings.connection")
                }
            }
            .scrollContentBackground(.hidden).background(PhoneTheme.base)
            .navigationTitle("Session settings").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .font(PhoneTheme.prose()).foregroundStyle(PhoneTheme.text)
        .presentationDetents([.fraction(0.8)]).presentationDragIndicator(.visible)
        .presentationCornerRadius(22)
    }
}
