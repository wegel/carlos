// Present the configured host's real session screen and native sheets.
import SwiftUI

struct PhoneRootView: View {
    let store: PhoneStore
    @State private var sessions = false
    @State private var connection = false
    @State private var settings = false

    var body: some View {
        Group {
            if let host = store.current {
                PhoneSessionView(host: host, sessions: { sessions = true }, settings: { settings = true })
            } else {
                ConnectionView(store: store, completed: { sessions = true })
            }
        }
        .background(PhoneTheme.mantle).foregroundStyle(PhoneTheme.text)
        .environment(store.preferences)
        .sheet(isPresented: $settings) {
            if let host = store.current {
                SessionSettingsSheet(preferences: store.preferences, host: host) { connection = true }
            }
        }
        .sheet(isPresented: $sessions) { SessionsSheet(store: store) }
        .sheet(isPresented: $connection) { ConnectionView(store: store, existing: store.current?.configuration) { connection = false } }
    }
}
