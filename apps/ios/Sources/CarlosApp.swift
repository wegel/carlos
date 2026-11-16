// Start the native phone shell; the shared Rust client owns all protocol checks.
import SwiftUI

@main
struct CarlosApp: App {
    var body: some Scene {
        WindowGroup {
            ConnectionView()
                .preferredColorScheme(.dark)
        }
    }
}

struct ConnectionView: View {
    @State private var address = ""
    @State private var host = ""
    @State private var token = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Carlos daemon") {
                    TextField("Host name", text: $host)
                    TextField("wss://host:port", text: $address)
                        .textContentType(.URL)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    SecureField("Device token", text: $token)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }
                Section {
                    Text("Use a device token provisioned by your Carlos daemon.")
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Carlos")
        }
    }
}
