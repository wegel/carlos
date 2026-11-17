// Check actual bundled fonts and simulator Keychain access, including ad-hoc signing.
import UIKit
import XCTest
@testable import Carlos

final class DeviceBoundaryTests: XCTestCase {
    func testFontsAndKeychainUseTheRealDeviceAPIs() async throws {
        try await MainActor.run {
            for name in ["InterVariable", "InterVariableItalic", "JetBrainsMono-Regular"] {
                XCTAssertNotNil(UIFont(name: name, size: 15), "Missing bundled font \(name)")
            }
            let prose = try XCTUnwrap(UIFont(name: "InterVariable", size: 15))
            XCTAssertEqual(prose.lineHeight + PhoneTheme.proseLeading, 22.5, accuracy: 0.05)
            let tool = try XCTUnwrap(UIFont(name: "InterVariable", size: 13.5))
            XCTAssertEqual(tool.lineHeight + PhoneTheme.toolLeading, 18.225, accuracy: 0.05)
            let account = "fixture-" + UUID().uuidString
            defer { try? HostTokens.remove(for: account) }
            try HostTokens.store("private synthetic token", for: account)
            XCTAssertEqual(try HostTokens.read(for: account), "private synthetic token")
            try HostTokens.store("updated synthetic token", for: account)
            XCTAssertEqual(try HostTokens.read(for: account), "updated synthetic token")
        }
    }

    func testCleartextAndPasswordURLsRequireTheActualConnectionRules() throws {
        let plain = PhoneHost(id: "test", name: "test", address: "ws://127.0.0.1:12345", allowsPrivateWebSocket: false)
        XCTAssertThrowsError(try plain.url())
        let explicit = PhoneHost(id: "test", name: "test", address: plain.address, allowsPrivateWebSocket: true)
        XCTAssertEqual(try explicit.url().scheme, "ws")
        let password = PhoneHost(id: "test", name: "test", address: "wss://name:secret@host", allowsPrivateWebSocket: false)
        XCTAssertThrowsError(try password.url())
    }
}
