// Dispatch actual physical key events through iOS, then check the original native prompt.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testHardwareReturnSendsAndModifiedReturnKeepsNewlines() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone")
        let oneLineHeight = draft.frame.height
        draft.typeKey("x", modifierFlags: [])
        XCTAssertEqual(draft.value as? String, "phonex", "A printable hardware key must reach the editor before testing Return")
        draft.typeText(XCUIKeyboardKey.delete.rawValue)
        XCTAssertEqual(draft.value as? String, "phone")
        draft.typeKey(XCUIKeyboardKey.return, modifierFlags: .shift)
        XCTAssertEqual(draft.value as? String, "phone\n")
        draft.typeText("keyboard")
        XCTAssertEqual(draft.value as? String, "phone\nkeyboard")
        draft.typeKey(XCUIKeyboardKey.return, modifierFlags: .option)
        XCTAssertEqual(draft.value as? String, "phone\nkeyboard\n")
        draft.typeText("exact 🦀")
        XCTAssertEqual(draft.value as? String, "phone\nkeyboard\nexact 🦀")
        XCTAssertGreaterThan(draft.frame.height, oneLineHeight + 20)
        XCTAssertTrue(app.buttons["composer.send"].isEnabled)
        capture("Modified hardware Return keeps a multiline draft", screenshot: app.screenshot())
        draft.typeKey(XCUIKeyboardKey.return, modifierFlags: [])
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Answer: phone'")).firstMatch.waitForExistence(timeout: 15))
        XCTAssertEqual(draft.value as? String, "")
        XCTAssertFalse(app.buttons["composer.send"].isEnabled)
        capture("Hardware Return sends exactly one native prompt", screenshot: app.screenshot())
        let unsent = "private\nsecond\nthird\nfourth\nfifth\nsixth 🦀"
        draft.typeText(unsent)
        XCTAssertEqual(draft.value as? String, unsent, "Text-entry newlines must stay in the draft")
        XCTAssertGreaterThan(draft.frame.height, 95)
        XCTAssertLessThanOrEqual(draft.frame.height, 137, "The composer must scroll internally beyond five lines")
        XCTAssertTrue(app.buttons["composer.send"].isEnabled)
        capture("A six-line private draft stops growing after five lines", screenshot: app.screenshot())
        app.terminate()
    }
}
