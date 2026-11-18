// Exercise generated Swift calls against the actual Rust library in the simulator.
import Foundation
import XCTest
@testable import Carlos

final class BridgeTests: XCTestCase {
    private let token = String(repeating: "1", count: 32) + "." + String(repeating: "2", count: 64)

    func testGreetingReconnectAndStaleCallbacks() throws {
        let client = PhoneClient()
        let first = try client.begin(name: "phone", token: token)
        let request = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(first.payload.utf8)) as? [String: Any])
        XCTAssertEqual(request["method"] as? String, "hello")
        XCTAssertTrue(try client.receive(generation: first.generation, message: Self.hello(first.requestId)).ready)
        try client.disconnect(generation: first.generation)
        let next = try client.begin(name: "phone", token: token)
        let stale = try client.receive(generation: first.generation, message: Data("not JSON".utf8))
        XCTAssertTrue(stale.outgoing.isEmpty)
        try client.disconnect(generation: first.generation)
        XCTAssertTrue(try client.receive(generation: next.generation, message: Self.hello(next.requestId, resumed: true)).ready)
    }

    func testTypedInvalidInputAndFrameErrors() throws {
        let client = PhoneClient()
        XCTAssertThrowsError(try client.begin(name: "phone", token: "private invalid token")) { error in
            guard case PhoneError.InvalidInput = error else { return XCTFail("Expected typed invalid input") }
        }
        let frame = try client.begin(name: "phone", token: token)
        XCTAssertThrowsError(try client.receive(generation: frame.generation, message: Data("bad".utf8))) { error in
            guard case PhoneError.InvalidMessage = error else { return XCTFail("Expected typed invalid message") }
        }
        XCTAssertEqual(client.maximumMessageBytes(), 8 * 1024 * 1024)
        XCTAssertEqual(PhoneFailureMessage.describe(PhoneError.StaleRead(message: "These questions changed; your answers are retained")),
                       "These questions changed; your answers are retained")
    }

    @MainActor
    func testRejectedControlsReleaseCapacityForANewPrompt() async throws {
        let id = UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: id))
        defer { defaults.removePersistentDomain(forName: id) }
        let host = PhoneHostModel(configuration: PhoneHost(id: id, name: "Fixture", address: "wss://example.test",
                                                           allowsPrivateWebSocket: false),
                                  defaults: defaults)
        let token = token
        try await read(host) { client in
            let frame = try client.begin(name: "phone", token: token)
            XCTAssertTrue(try client.receive(generation: frame.generation, message: Self.hello(frame.requestId)).ready)
        }

        // The real Rust client holds at most 128 operations. Every rejected control
        // must be released after Swift surfaces the error, without restarting it.
        for _ in 0..<129 {
            let operation = try await read(host) { client in
                let submission = try client.createSession(backend: .codex, cwd: "/project")
                let frame = submission.frame
                let rejection = try JSONSerialization.data(withJSONObject: [
                    "jsonrpc": "2.0", "id": frame.requestId,
                    "error": ["code": -32000, "message": "Fixture rejected control"]
                ])
                let batch = try client.receive(generation: frame.generation, message: rejection)
                XCTAssertEqual(batch.operations, [submission.operation.key])
                return try XCTUnwrap(client.operation(operation: submission.operation.key))
            }
            host.accept(operation)
            XCTAssertTrue(host.error?.contains("Fixture rejected control") == true)
        }

        let prompt = try await read(host) { client in
            XCTAssertTrue(try client.operations().isEmpty)
            let session = String(repeating: "a", count: 32)
            let info: [String: Any] = [
                "id": session, "name": "Fixture session", "backend": "codex", "host": "fixture",
                "cwd": "/project", "phase": "idle", "availability": "ready", "ralph": false
            ]
            try Self.reply(client, to: client.watchSessions(), result: ["sessions": [info]])
            try Self.reply(client, to: client.subscribe(session: session), result: [
                "kind": "snapshot", "snapshot": ["info": info, "entries": [], "approvals": [],
                                                "seq": "1", "has_older": false]
            ])
            return try client.sendPrompt(session: session, text: "still usable 🌍", mode: .steer)
        }
        XCTAssertTrue(host.operations.isEmpty)
        XCTAssertEqual(prompt.operation.text, "still usable 🌍")
        XCTAssertTrue(prompt.operation.needsEcho)
    }

    @MainActor
    private func read<Value: Sendable>(
        _ host: PhoneHostModel, _ call: @escaping @Sendable (PhoneClient) throws -> Value
    ) async throws -> Value {
        try await withCheckedThrowingContinuation { continuation in
            host.transport.read(call) { continuation.resume(with: $0) }
        }
    }

    private static func reply(_ client: PhoneClient, to frame: PhoneFrame, result: [String: Any]) throws {
        let bytes = try JSONSerialization.data(withJSONObject: ["jsonrpc": "2.0", "id": frame.requestId, "result": result])
        XCTAssertTrue(try client.receive(generation: frame.generation, message: bytes).ready)
    }

    private static func hello(_ id: Int64, resumed: Bool = false) throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "jsonrpc": "2.0", "id": id,
            "result": ["protocol": 1, "client": String(repeating: "3", count: 32),
                       "instance": String(repeating: "4", count: 32),
                       "resume_token": String(repeating: "5", count: 64),
                       "next_operation": "1", "resumed": resumed,
                       "host": "fixture", "daemon_version": "0.1.0"]
        ])
    }
}
