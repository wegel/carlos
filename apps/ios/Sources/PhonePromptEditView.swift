// Edit an earlier prompt without replacing the normal composer or starting work on sheet open.
import SwiftUI
import UIKit

struct PhonePromptEditView: View {
    let host: PhoneHostModel
    @Bindable var editor: PhonePromptEditor
    @Environment(\.dismiss) private var dismiss

    private var running: Bool { host.transcript?.info.activity.phase != .idle }
    private var canSend: Bool {
        editor.loaded && !editor.sending && editor.operation == nil && host.connected &&
            host.transcript?.ready == true && !editor.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("Start a new branch from before this message.")
                    .font(PhoneTheme.prose(14)).foregroundStyle(PhoneTheme.subtext1)
                if editor.loading { ProgressView("Reading original message…") }
                TextEditor(text: $editor.text)
                    .font(PhoneTheme.mono(14)).scrollContentBackground(.hidden)
                    .disabled(!editor.loaded || editor.sending || editor.operation != nil)
                    .accessibilityIdentifier("edit.draft")
                if let error = editor.error {
                    Text(PhoneFailureMessage.describeForkFailure(error)).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                        .accessibilityIdentifier("edit.error")
                }
                if let operation = editor.operation {
                    switch operation.state {
                    case .rejected, .uncertain: PhoneRecoveryView(host: host, operation: operation)
                    default: ProgressView("Preparing branch…")
                    }
                } else if running {
                    Text("Stop the current response before creating this branch.")
                        .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext1)
                    Button(role: .destructive) { host.interrupt() } label: {
                        Text("Stop response").frame(maxWidth: .infinity, minHeight: 28)
                    }
                        .buttonStyle(.borderedProminent).controlSize(.large)
                        .tint(PhoneTheme.red).foregroundStyle(PhoneTheme.crust)
                        .disabled(!host.connected || host.controlSubmitting || host.transcript?.currentSnapshot != true || host.transcript?.info.activity.canInterrupt != true)
                        .accessibilityIdentifier("edit.stop")
                } else {
                    Button { editor.submit(host: host) } label: {
                        Text("Fork and send").frame(maxWidth: .infinity, minHeight: 28)
                    }
                        .buttonStyle(.borderedProminent).controlSize(.large)
                        .foregroundStyle(PhoneTheme.crust)
                        .disabled(!canSend).accessibilityIdentifier("edit.send")
                }
                if !editor.loaded && !editor.loading {
                    Button("Retry reading message") { editor.load() }
                        .disabled(!host.connected).accessibilityIdentifier("edit.retry")
                }
            }
            .padding(16).background(PhoneTheme.mantle)
            .navigationTitle("Edit message").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }.disabled(editor.sending)
                        .accessibilityIdentifier("edit.close")
                }
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button("Done") {
                        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                    }
                }
            }
        }
        .tint(PhoneTheme.blue).interactiveDismissDisabled(editor.sending)
    }
}
