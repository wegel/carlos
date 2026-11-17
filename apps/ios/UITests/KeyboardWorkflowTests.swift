// Dispatch actual HID key events through iOS, then check the original native prompt.
import Foundation
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testHardwareReturnSendsAndModifiedReturnKeepsNewlines() async throws {
        let app = try connectedSession()
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons["Full"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'history-output 0'")).firstMatch.waitForExistence(timeout: 15),
                      "Full mode must remain responsive with the real ten-thousand-line native output")
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        try checkIdleComposerCaret(app, draft: draft)
        draft.tap()
        draft.typeText("phone")
        XCTAssertEqual(draft.value as? String, "phone", "A native edit must survive an unrelated SwiftUI refresh")
        let oneLineHeight = draft.frame.height
        draft.typeKey("x", modifierFlags: [])
        XCTAssertEqual(draft.value as? String, "phonex", "A printable hardware key must reach the editor before testing Return")
        draft.typeText(XCUIKeyboardKey.delete.rawValue)
        XCTAssertEqual(draft.value as? String, "phone")
        try await rawReturn("x")
        XCTAssertTrue(wait { draft.value as? String == "phonex" }, "The raw HID channel must deliver an ordinary key before testing Return")
        draft.typeText(XCUIKeyboardKey.delete.rawValue)
        XCTAssertEqual(draft.value as? String, "phone")
        try await rawReturn("shift")
        XCTAssertTrue(wait { draft.value as? String == "phone\n" }, "Wait for the externally injected native press to reach UIKit")
        draft.typeText("keyboard")
        XCTAssertEqual(draft.value as? String, "phone\nkeyboard")
        try await rawReturn("option")
        XCTAssertTrue(wait { draft.value as? String == "phone\nkeyboard\n" })
        draft.typeText("exact 🦀")
        XCTAssertEqual(draft.value as? String, "phone\nkeyboard\nexact 🦀")
        XCTAssertGreaterThan(draft.frame.height, oneLineHeight + 20)
        XCTAssertTrue(app.buttons["composer.send"].isEnabled)
        capture("Modified hardware Return keeps a multiline draft", screenshot: app.screenshot())
        try await rawReturn("plain")
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Answer: phone'")).firstMatch.waitForExistence(timeout: 15))
        XCTAssertEqual(draft.value as? String, "")
        XCTAssertFalse(app.buttons["composer.send"].isEnabled)
        capture("Hardware Return sends exactly one native prompt", screenshot: app.screenshot())
        let unsent = "private\nsecond\nthird\nfourth\nfifth\nsixth 🦀"
        draft.typeText("private\nsecond\nthird\nfourth\nfifth")
        let fiveLineHeight = draft.frame.height
        draft.typeText("\nsixth 🦀")
        XCTAssertEqual(draft.value as? String, unsent, "Text-entry newlines must stay in the draft")
        XCTAssertEqual(draft.frame.height, fiveLineHeight, accuracy: 0.5,
                       "The sixth line must scroll inside the existing five-line input area")
        XCTAssertGreaterThan(draft.frame.height, 95)
        XCTAssertLessThanOrEqual(draft.frame.height, 137, "The composer must scroll internally beyond five lines")
        XCTAssertTrue(app.buttons["composer.send"].isEnabled)
        capture("A six-line private draft stops growing after five lines", screenshot: app.screenshot())
        let done = app.buttons["composer.keyboardDone"]
        XCTAssertTrue(done.waitForExistence(timeout: 15))
        done.tap()
        XCTAssertTrue(wait { !app.keyboards.firstMatch.exists }, "Done must close the actual software keyboard")
        XCTAssertEqual(draft.value as? String, unsent, "Closing the keyboard must keep the private draft")
        try checkIdleComposerCaret(app, draft: draft)
        capture("Keyboard closes while the flat draft and insertion point remain", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor private func rawReturn(_ modifier: String) async throws {
        struct Keyboard: Decodable { let keyboardControlURL: URL }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let control = try JSONDecoder().decode(Keyboard.self, from: Data(contentsOf: file))
        var request = URLRequest(url: control.keyboardControlURL.appendingPathComponent(modifier))
        request.httpMethod = "POST"
        let (_, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200, "The raw HID event must reach the owned simulator")
    }
}
