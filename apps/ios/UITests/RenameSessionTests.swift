// Exercise cancellation, rejection and accepted names through the actual native backend.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testRenameWaitsForAcknowledgementAndKeepsTheMainDraft() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("keep my prompt")
        app.buttons["Hide keyboard"].firstMatch.tap()
        app.buttons["header.settings"].tap()
        let rename = app.buttons["settings.rename"]
        XCTAssertTrue(rename.waitForExistence(timeout: 15))
        rename.tap()
        let name = app.textFields["rename.name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("cancelled private title")
        app.buttons["rename.cancel"].tap()
        rename.tap()
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        XCTAssertEqual(name.value as? String, "Session name")
        name.tap()
        name.typeText("reject rename")
        app.buttons["rename.save"].tap()
        let error = app.staticTexts["rename.error"]
        XCTAssertTrue(error.waitForExistence(timeout: 15))
        XCTAssertTrue(error.label.contains("fixture rejected rename"))
        XCTAssertEqual(name.value as? String, "reject rename")
        XCTAssertFalse(name.isEnabled, "An uncertain backend outcome must not offer an immediate retry")
        XCTAssertFalse(app.buttons["rename.save"].isEnabled)
        capture("Unconfirmed rename retains the private name", screenshot: app.screenshot())
        app.buttons["rename.cancel"].tap()
        app.buttons["Done"].firstMatch.tap()
        XCTAssertTrue(app.buttons["header.title"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["header.title"].label.contains("reject rename"))
        XCTAssertEqual(draft.value as? String, "keep my prompt")
        app.buttons["header.settings"].tap()
        rename.tap()
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Phone project 🌍")
        app.buttons["rename.save"].tap()
        XCTAssertTrue(wait { !app.buttons["rename.cancel"].exists })
        XCTAssertTrue(app.staticTexts["Phone project 🌍"].waitForExistence(timeout: 15))
        app.buttons["Done"].firstMatch.tap()
        XCTAssertTrue(app.buttons["header.title"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["header.title"].label.contains("Phone project 🌍"))
        XCTAssertEqual(draft.value as? String, "keep my prompt")
        XCTAssertFalse(app.staticTexts["session.error"].exists, "A successful retry must not leave the old rename error on the conversation")
        capture("Confirmed name keeps the private composer", screenshot: app.screenshot())
        app.buttons["header.sessions"].tap()
        XCTAssertTrue(app.staticTexts["Phone project 🌍"].waitForExistence(timeout: 15))
        app.terminate()
    }
}
