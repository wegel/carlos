// Verify actual Steer/Queue requests and old-message branches without losing another composer draft.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testSteerAndQueueKeepTheirChosenNativeMeaning() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone mode run")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.buttons["composer.stop"].waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone steer exact 🌍")
        XCTAssertFalse(app.buttons["composer.stop"].exists, "Stop stays out of the draft's send action")
        XCTAssertEqual(app.buttons["composer.mode"].value as? String, "Steer")
        let done = app.buttons["composer.keyboardDone"]
        let microphone = app.buttons["composer.microphone"]
        XCTAssertTrue(done.exists && microphone.exists)
        XCTAssertEqual(microphone.frame.midY, done.frame.midY, accuracy: 2)
        XCTAssertEqual(app.buttons["composer.mode"].frame.midY, done.frame.midY, accuracy: 2)
        XCTAssertGreaterThan(done.frame.minY, app.buttons["composer.send"].frame.midY)
        capture("Explicit Steer beside a private draft", screenshot: app.screenshot())
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Steer accepted'")).firstMatch.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone queued exact 🌍")
        chooseSendMode("Queue for the next turn", app: app)
        app.buttons["composer.send"].tap()
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(wait { stop.isEnabled })
        stop.tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Answer: phone queued exact 🌍'")).firstMatch.waitForExistence(timeout: 15))
        XCTAssertEqual(draft.value as? String, "")
        capture("Queued message runs after the original Stop", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testEditingAnEarlierMessageKeepsTheMainDraft() throws {
        let app = try preparedOldMessageEdit()
        let edited = app.textViews["edit.draft"]
        replaceOldPrompt(edited, with: "phone edited exact 🌍")
        app.buttons["Done"].firstMatch.tap()
        capture("A separate editor keeps the normal draft", screenshot: app.screenshot())
        XCTAssertGreaterThanOrEqual(app.buttons["edit.send"].frame.height, 44)
        app.buttons["edit.send"].tap()
        XCTAssertTrue(wait { !app.buttons["edit.close"].exists })
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Answer: phone edited exact 🌍'")).firstMatch.waitForExistence(timeout: 15))
        XCTAssertEqual(app.textViews["composer.draft"].value as? String, "keep original composer")
        XCTAssertFalse(app.staticTexts["Original answer before branch"].exists)
        capture("Edited branch and untouched main composer", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testRejectedBranchKeepsEditedTextAndTheMainDraft() throws {
        let app = try preparedOldMessageEdit()
        replaceOldPrompt(app.textViews["edit.draft"], with: "phone rejected edit 🌍")
        app.buttons["Done"].firstMatch.tap()
        app.buttons["edit.send"].tap()
        let failure = app.staticTexts["edit.error"]
        XCTAssertTrue(failure.waitForExistence(timeout: 15))
        XCTAssertTrue(failure.label.contains("Phone branch rejected"))
        let restore = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'recovery.restore.'")).firstMatch
        XCTAssertTrue(restore.waitForExistence(timeout: 15))
        XCTAssertFalse(restore.isEnabled, "A failed edit must not replace a different draft")
        XCTAssertEqual(app.textViews["edit.draft"].value as? String, "phone rejected edit 🌍")
        capture("Rejected fork retains both drafts", screenshot: app.screenshot())
        app.buttons["edit.close"].tap()
        XCTAssertEqual(app.textViews["composer.draft"].value as? String, "keep original composer")
        XCTAssertTrue(app.staticTexts["Original answer before branch"].exists)
        app.terminate()
    }

    @MainActor
    private func preparedOldMessageEdit() throws -> XCUIApplication {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("keep original composer")
        app.buttons["Hide keyboard"].firstMatch.tap()
        openOldMessageEditor(app)
        replaceOldPrompt(app.textViews["edit.draft"], with: "cancelled private edit")
        app.buttons["edit.close"].tap()
        XCTAssertEqual(draft.value as? String, "keep original composer")
        openOldMessageEditor(app)
        return app
    }

    @MainActor
    private func openOldMessageEditor(_ app: XCUIApplication) {
        let row = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'entry.user.'")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 15))
        row.press(forDuration: 1)
        let edit = app.buttons["Edit from here"].firstMatch
        XCTAssertTrue(edit.waitForExistence(timeout: 5))
        edit.tap()
        let field = app.textViews["edit.draft"]
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        XCTAssertTrue(wait { field.value as? String == "original prompt" })
    }

    @MainActor
    private func replaceOldPrompt(_ field: XCUIElement, with text: String) {
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: "original prompt".count))
        field.typeText(text)
        XCTAssertEqual(field.value as? String, text)
    }

    @MainActor
    private func chooseSendMode(_ title: String, app: XCUIApplication) {
        let mode = app.buttons["composer.mode"]
        XCTAssertTrue(mode.waitForExistence(timeout: 15))
        mode.tap()
        let choice = app.buttons[title].firstMatch
        XCTAssertTrue(choice.waitForExistence(timeout: 5))
        choice.tap()
    }
}
