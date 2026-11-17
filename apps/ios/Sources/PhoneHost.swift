// Persist host addresses separately from device tokens kept in this app's Keychain service.
import Foundation
import Security

struct PhoneHost: Codable, Identifiable, Sendable {
    let id: String
    let name: String
    let address: String
    let allowsPrivateWebSocket: Bool

    func url() throws -> URL {
        guard let parts = URLComponents(string: address), let url = parts.url,
              let host = parts.host, !host.isEmpty, parts.user == nil, parts.password == nil,
              parts.fragment == nil, parts.scheme == "wss" || parts.scheme == "ws" else {
            throw HostError.message("Enter a complete ws:// or wss:// daemon URL without a password")
        }
        if parts.scheme == "ws" && !allowsPrivateWebSocket {
            throw HostError.message("Enable private-network WebSocket to use an unencrypted connection")
        }
        if let port = parts.port, !(1...65535).contains(port) {
            throw HostError.message("Enter a port between 1 and 65535")
        }
        return url
    }
}

enum HostError: LocalizedError {
    case message(String)
    case keychain(OSStatus)

    var errorDescription: String? {
        switch self {
        case .message(let text): text
        case .keychain(errSecItemNotFound): "Enter a device token for this host"
        case .keychain(errSecInteractionNotAllowed): "Unlock this iPhone to use its saved token"
        case .keychain(let code): "Cannot access this device token (Keychain \(code))"
        }
    }
}

enum HostTokens {
    private static let service = "app.carlos.phone.hosts"

    static func store(_ token: String, for host: String) throws {
        let value = Data(token.utf8)
        let query = identity(host)
        let status = SecItemUpdate(query as CFDictionary, [kSecValueData: value] as CFDictionary)
        if status == errSecItemNotFound {
            var item = query
            item[kSecValueData] = value
            item[kSecAttrAccessible] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            try check(SecItemAdd(item as CFDictionary, nil))
        } else { try check(status) }
    }

    static func read(for host: String) throws -> String {
        var query = identity(host)
        query[kSecReturnData] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        try check(SecItemCopyMatching(query as CFDictionary, &result))
        guard let data = result as? Data, let token = String(data: data, encoding: .utf8) else {
            throw HostError.message("Enter a device token for this host")
        }
        return token
    }

    static func remove(for host: String) throws {
        let status = SecItemDelete(identity(host) as CFDictionary)
        if status != errSecItemNotFound { try check(status) }
    }

    private static func identity(_ host: String) -> [CFString: Any] {
        [kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecAttrAccount: host]
    }

    private static func check(_ status: OSStatus) throws {
        if status != errSecSuccess { throw HostError.keychain(status) }
    }
}
