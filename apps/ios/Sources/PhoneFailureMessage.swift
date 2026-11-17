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
}
