// Show Rust's actionable message without generated Swift type names or enum syntax.
import Foundation

enum PhoneFailureMessage {
    static func describe(_ error: any Error) -> String {
        guard let error = error as? PhoneError else { return error.localizedDescription }
        switch error {
        case .InvalidInput(let message), .Capacity(let message), .StaleRead(let message),
             .State(let message), .InvalidMessage(let message), .Internal(let message):
            return message
        }
    }

    // Keep the operation's original error intact for diagnostics.
    static func describeForkFailure(_ diagnostic: String) -> String {
        let prefix = "server returned error: "
        guard diagnostic.hasPrefix(prefix),
              let error = try? JSONDecoder().decode(ServerError.self, from: Data(diagnostic.dropFirst(prefix.count).utf8)),
              !error.message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return diagnostic }
        return error.message
    }

    private struct ServerError: Decodable {
        let code: Int
        let message: String
    }
}
