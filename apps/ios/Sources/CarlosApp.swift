// Start the native phone shell; the shared Rust client owns all protocol checks.
import SwiftUI

@main
struct CarlosApp: App {
    @State private var store = PhoneStore()
    var body: some Scene {
        WindowGroup {
            PhoneRootView(store: store)
                .preferredColorScheme(.dark)
        }
    }
}
