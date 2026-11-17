// A new draft always steers by default, even when an old app saved Queue as its preference.
import Foundation
import XCTest
@testable import Carlos

final class HostInputTests: XCTestCase {
    func testQueueAppliesOnlyToTheCurrentDraft() async throws {
        try await MainActor.run {
            let suite = "input-" + UUID().uuidString
            let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
            defer { defaults.removePersistentDomain(forName: suite) }
            let configuration = PhoneHost(id: "input-host", name: "Host", address: "wss://example.test", allowsPrivateWebSocket: false)
            defaults.set("queue", forKey: "carlos.hosts.input-host.sendMode")
            let host = PhoneHostModel(configuration: configuration, defaults: defaults)
            XCTAssertEqual(host.sendMode, .steer)
            host.editDraft("first message")
            host.sendMode = .queue
            host.editDraft("first message revised")
            XCTAssertEqual(host.sendMode, .queue)
            host.editDraft("")
            XCTAssertEqual(host.sendMode, .steer)
            host.draft = "second message"
            host.sendMode = .queue
            host.draft = ""
            XCTAssertEqual(host.sendMode, .steer)
        }
    }
}
