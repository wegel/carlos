// Present the configured host's real session screen and native sheets.
import SwiftUI

struct PhoneRootView: View {
    let store: PhoneStore
    @State private var sessions = false
    @State private var connection = false

    var body: some View {
        Group {
            if let host = store.current {
                PhoneSessionView(host: host, sessions: { sessions = true }, connection: { connection = true })
            } else {
                ConnectionView(store: store, completed: { sessions = true })
            }
        }
        .background(PhoneTheme.mantle).foregroundStyle(PhoneTheme.text)
        .sheet(isPresented: $sessions) { SessionsSheet(store: store) }
        .sheet(isPresented: $connection) { ConnectionView(store: store, existing: store.current?.configuration) { connection = false } }
    }
}
