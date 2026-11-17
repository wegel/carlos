// Start the native phone shell; the shared Rust client owns all protocol checks.
import SwiftUI

@main
struct CarlosApp: App {
    @State private var store = PhoneStore()
    @State private var reconnectAfterBackground = false
    @Environment(\.scenePhase) private var scenePhase
    var body: some Scene {
        WindowGroup {
            PhoneRootView(store: store)
                .preferredColorScheme(.dark)
                .task { PhoneScrollRecording.shared.startIfRequested() }
                .onChange(of: scenePhase, initial: true) { _, phase in
                    if phase == .background { reconnectAfterBackground = true }
                    if phase == .active { resumeConnections() }
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.protectedDataDidBecomeAvailableNotification)) { _ in
                    if scenePhase == .active { resumeConnections() }
                }
        }
    }

    private func resumeConnections() {
        guard UIApplication.shared.isProtectedDataAvailable else { return }
        store.resumeConnections(force: reconnectAfterBackground)
        reconnectAfterBackground = false
    }
}
