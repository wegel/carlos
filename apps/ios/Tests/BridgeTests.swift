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
        XCTAssertTrue(try client.receive(generation: first.generation, message: hello(first.requestId)).ready)
        try client.disconnect(generation: first.generation)
        let next = try client.begin(name: "phone", token: token)
        let stale = try client.receive(generation: first.generation, message: Data("not JSON".utf8))
        XCTAssertTrue(stale.outgoing.isEmpty)
        try client.disconnect(generation: first.generation)
        XCTAssertTrue(try client.receive(generation: next.generation, message: hello(next.requestId, resumed: true)).ready)
    }

    func testTypedInvalidInputAndFrameErrors() throws {
        let client = PhoneClient()
        XCTAssertThrowsError(try client.begin(name: "phone", token: "private invalid token")) { error in
            guard case PhoneError.InvalidInput = error else { return XCTFail("Expected typed invalid input") }
        }
        let frame = try client.begin(name: "phone", token: token)
        XCTAssertThrowsError(try client.receive(generation: frame.generation, message: Data("bad".utf8))) { error in
            guard case PhoneError.Protocol = error else { return XCTFail("Expected typed protocol error") }
        }
        XCTAssertEqual(client.maximumMessageBytes(), 8 * 1024 * 1024)
    }

    private func hello(_ id: Int64, resumed: Bool = false) throws -> Data {
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
