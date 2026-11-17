// Rename a session without replacing the conversation's normal input draft.
import SwiftUI

struct RenameSessionSheet: View {
    let host: PhoneHostModel
    @Bindable var editor: PhoneSessionRename
    @Environment(\.dismiss) private var dismiss
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Session name", text: $editor.name)
                        .font(PhoneTheme.prose()).textInputAutocapitalization(.sentences)
                        .autocorrectionDisabled().focused($focused).submitLabel(.done)
                        .disabled(editor.sending || editor.uncertain)
                        .accessibilityIdentifier("rename.name")
                } footer: {
                    Text("Saved on your computer and shared with your other devices. Leave empty to use the automatic title.")
                }
                if let error = editor.error {
                    Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                        .accessibilityIdentifier("rename.error")
                }
                if editor.uncertain {
                    Text("Check the session name after reconnecting before trying again.")
                        .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext1)
                }
            }
            .scrollContentBackground(.hidden).background(PhoneTheme.base)
            .navigationTitle("Rename session").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }.disabled(editor.sending)
                        .accessibilityIdentifier("rename.cancel")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button { focused = false; editor.submit(host: host) } label: {
                        if editor.sending { ProgressView() } else { Text("Save") }
                    }
                    .disabled(editor.sending || editor.uncertain || !host.connected || host.transcript?.ready != true)
                    .accessibilityIdentifier("rename.save")
                }
            }
        }
        .tint(PhoneTheme.blue).foregroundStyle(PhoneTheme.text)
        .interactiveDismissDisabled(editor.sending)
        .onAppear { focused = true }
    }
}
