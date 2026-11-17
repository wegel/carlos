// Keep private edits on a changed request until the person explicitly starts new answers.
import Foundation
import XCTest
@testable import Carlos

final class QuestionDraftTests: XCTestCase {
    private let session = String(repeating: "a", count: 32)
    private let card = String(repeating: "b", count: 32) + "-0000000000000001"

    @MainActor
    func testChangedRequestKeepsPrivateAnswersUntilExplicitRestart() async throws {
        let client = try connected()
        let subscribe = try client.subscribe(session: session)
        try receive(client, ["id": subscribe.requestId, "result": ["kind": "snapshot", "snapshot": [
            "info": ["id": session, "name": "Form", "backend": "codex", "host": "fixture", "cwd": "/project",
                     "phase": "approval", "availability": "ready", "ralph": false],
            "seq": "1", "entries": [], "approvals": [approval("Original count")], "has_older": false
        ]]])
        let draft = PhoneQuestionDraft(card: card, editor: try editor(client))
        await draft.refresh(current: draft.editor)
        draft.notes["count"] = "10"
        try receive(client, ["method": "session/event", "params": ["session": session, "seq": "2", "event": [
            "kind": "approval_requested", "approval": approval("Updated count"), "before": NSNull()
        ]]])
        let current = try editor(client)
        await draft.refresh(current: current)
        XCTAssertTrue(draft.changed)
        XCTAssertEqual(draft.notes["count"], "10")
        XCTAssertEqual(draft.fields.first?.title, "Original count")
        XCTAssertThrowsError(try client.answerQuestions(questions: draft.editor, answers: draft.answers()))
        XCTAssertTrue(try client.operations().isEmpty)
        await draft.restart(current: current)
        XCTAssertFalse(draft.changed)
        XCTAssertTrue(draft.notes.isEmpty)
        XCTAssertEqual(draft.fields.first?.title, "Updated count")
        draft.notes["count"] = "5"
        let submitted = try client.answerQuestions(questions: draft.editor, answers: draft.answers())
        let request = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(submitted.frame.payload.utf8)) as? [String: Any])
        let params = try XCTUnwrap(request["params"] as? [String: Any])
        let answer = try XCTUnwrap(params["answer"] as? [String: Any])
        XCTAssertEqual((answer["value"] as? [String: Int])?["count"], 5)
    }

    private func connected() throws -> PhoneClient {
        let client = PhoneClient()
        let frame = try client.begin(name: "phone", token: String(repeating: "1", count: 32) + "." + String(repeating: "2", count: 64))
        try receive(client, ["id": frame.requestId, "result": [
            "protocol": 1, "client": String(repeating: "3", count: 32), "instance": String(repeating: "4", count: 32),
            "resume_token": String(repeating: "5", count: 64), "next_operation": "1", "resumed": false,
            "host": "fixture", "daemon_version": "0.1.0"
        ]])
        return client
    }

    private func approval(_ title: String) -> [String: Any] {
        ["id": card, "kind": "command", "title": "Form", "details": [], "choices": ["deny", "cancel"],
         "active": true, "submitted": false, "input": ["kind": "form", "schema": [
            "type": "object", "required": ["count"], "properties": ["count": ["type": "integer", "title": title]]
         ]]]
    }

    private func editor(_ client: PhoneClient) throws -> PhoneQuestions {
        let card = try XCTUnwrap(client.approvals(session: session).first)
        guard case .form(let editor)? = card.input else {
            XCTFail("Expected the actual Rust form editor")
            throw CocoaError(.coderInvalidValue)
        }
        return editor
    }

    private func receive(_ client: PhoneClient, _ fields: [String: Any]) throws {
        var frame = fields
        frame["jsonrpc"] = "2.0"
        _ = try client.receive(generation: 1, message: JSONSerialization.data(withJSONObject: frame))
    }
}
