// Keep server edits attached to their entry and remove only that entry's device-local credentials.
import Foundation
import XCTest
@testable import Carlos

final class HostStoreTests: XCTestCase {
    func testEquivalentAddressesCannotCreateAnotherSavedServer() async throws {
        try await MainActor.run {
            let host = savedHost(address: "wss://example.test")
            try withSavedHosts([host]) { store, _ in
                XCTAssertThrowsError(try store.beginConnection(name: "Alpha", address: "wss://EXAMPLE.test:443/",
                    token: "explicit token", privateWebSocket: false)) { error in
                    XCTAssertTrue(error.localizedDescription.contains("already saved"))
                }
                XCTAssertEqual(store.hosts.count, 1)
                XCTAssertEqual(store.current?.id, host.id)
            }
        }
    }

    func testChangedAddressKeepsIdentityAndWorkingEntryUntilItAuthenticates() async throws {
        try await MainActor.run {
            let host = savedHost(address: "ws://127.0.0.1:1")
            try withSavedHosts([host]) { store, _ in
                let original = try XCTUnwrap(store.current)
                original.draft = "Keep this private draft"
                let attempt = try store.beginConnection(name: "Alpha", address: "ws://127.0.0.1:2",
                    token: String(repeating: "1", count: 32) + "." + String(repeating: "2", count: 64),
                    privateWebSocket: true, existingID: host.id)
                defer { attempt.cancel() }
                XCTAssertEqual(attempt.host.id, host.id)
                XCTAssertFalse(attempt.host === original)
                XCTAssertTrue(store.current === original)
                XCTAssertEqual(original.configuration.address, host.address)
                XCTAssertEqual(original.draft, "Keep this private draft")
                XCTAssertNil(attempt.host.selected)
                XCTAssertEqual(store.hosts.count, 1)
            }
        }
    }

    func testChangedAddressCannotReuseTheOldServersToken() async throws {
        try await MainActor.run {
            let host = savedHost(address: "wss://old.example.test")
            defer { try? HostTokens.remove(for: host.id) }
            try HostTokens.store("private old token", for: host.id)
            try withSavedHosts([host]) { store, _ in
                XCTAssertThrowsError(try store.beginConnection(name: "Alpha", address: "wss://new.example.test",
                    token: "", privateWebSocket: false, existingID: host.id)) { error in
                    XCTAssertTrue(error.localizedDescription.contains("Enter a device token"))
                }
                XCTAssertEqual(try HostTokens.read(for: host.id), "private old token")
                XCTAssertEqual(store.current?.configuration.address, host.address)
            }
        }
    }

    func testRemovalKeepsOtherHostsAndRejectsADeletedEditorsLateSave() async throws {
        try await MainActor.run {
            let first = savedHost(address: "wss://first.example.test")
            let second = savedHost(address: "wss://second.example.test")
            defer { try? HostTokens.remove(for: first.id); try? HostTokens.remove(for: second.id) }
            try HostTokens.store("first private token", for: first.id)
            try HostTokens.store("second private token", for: second.id)
            try withSavedHosts([first, second]) { store, defaults in
                let old = try XCTUnwrap(store.hosts.first)
                let late = PhoneConnectionAttempt(host: old, configuration: first, identity: UUID(), token: nil, existingID: first.id)
                store.selectedHost = first.id
                defaults.set("remembered session", forKey: "carlos.hosts.\(first.id).session")
                try store.removeHost(first.id)
                XCTAssertThrowsError(try HostTokens.read(for: first.id))
                XCTAssertEqual(try HostTokens.read(for: second.id), "second private token")
                XCTAssertEqual(store.current?.id, second.id)
                XCTAssertNil(defaults.object(forKey: "carlos.hosts.\(first.id).session"))
                XCTAssertThrowsError(try store.saveConnection(late)) { error in
                    XCTAssertTrue(error.localizedDescription.contains("removed"))
                }
                XCTAssertEqual(PhoneStore(defaults: defaults).hosts.map(\.id), [second.id])
                try store.removeHost(second.id)
                XCTAssertNil(store.current)
                XCTAssertTrue(PhoneStore(defaults: defaults).hosts.isEmpty)
                XCTAssertThrowsError(try HostTokens.read(for: second.id))
            }
        }
    }

    func testAddressKeysKeepDifferentPortsAndPathsSeparate() {
        XCTAssertEqual(PhoneHost.connectionKey("ws://Example.test:80/"), PhoneHost.connectionKey("ws://example.test"))
        XCTAssertNotEqual(PhoneHost.connectionKey("ws://example.test:33731"), PhoneHost.connectionKey("ws://example.test:33732"))
        XCTAssertNotEqual(PhoneHost.connectionKey("wss://example.test/one"), PhoneHost.connectionKey("wss://example.test/two"))
    }
}

private func savedHost(address: String) -> PhoneHost {
    PhoneHost(id: UUID().uuidString, name: "Alpha", address: address, allowsPrivateWebSocket: true)
}

@MainActor
private func withSavedHosts(_ hosts: [PhoneHost], check: (PhoneStore, UserDefaults) throws -> Void) throws {
    let suite = "hosts-" + UUID().uuidString
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    defaults.set(try JSONEncoder().encode(hosts), forKey: "carlos.hosts.v1")
    try check(PhoneStore(defaults: defaults), defaults)
}
