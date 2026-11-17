// Copy exact original text, including content outside the visible display window.
import SwiftUI
import UIKit
import OSLog

enum PhoneCopySource: Sendable {
    case inline(String)
    case original(PhoneSourceFile, UInt64)

    var byteCount: UInt64 {
        switch self { case .inline(let text): UInt64(text.utf8.count); case .original(_, let end): end }
    }

    func text() throws -> String {
        switch self {
        case .inline(let text): text
        case .original(let file, let end): try file.text(upTo: end)
        }
    }
}

struct PhoneCopyButton: View {
    private static let logger = Logger(subsystem: "app.carlos.phone", category: "clipboard")
    let source: PhoneCopySource
    @State private var copied = false
    @State private var copying = false
    @State private var error: String?
    @State private var notice = UUID()

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            Button {
                Self.logger.info("Copy requested, bytes=\(source.byteCount, privacy: .public)")
                copying = true
                let source = source
                Task {
                    let result = await Task.detached(priority: .userInitiated) { Result { try source.text() } }.value
                    copying = false
                    switch result {
                    case .success(let text):
                        Self.logger.info("Original read complete; writing pasteboard")
                        UIPasteboard.general.string = text
                        Self.logger.info("Pasteboard write complete")
                        copied = true
                        notice = UUID()
                    case .failure(let failure): error = PhoneFailureMessage.describe(failure)
                    }
                }
            } label: {
                Label(copied ? "Copied" : copying ? "Copying…" : "Copy", systemImage: copied ? "checkmark" : "document.on.document")
                    .font(PhoneTheme.mono(11.5)).frame(minWidth: 44, minHeight: 44)
            }
            .disabled(copying).foregroundStyle(PhoneTheme.overlay2).accessibilityIdentifier("answer.copy")
            .accessibilityValue(Text(verbatim: "\(source.byteCount) bytes"))
            if let error { Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red) }
        }
        .task(id: notice) {
            guard copied else { return }
            try? await Task.sleep(for: .milliseconds(1400))
            if !Task.isCancelled { copied = false }
        }
    }
}
