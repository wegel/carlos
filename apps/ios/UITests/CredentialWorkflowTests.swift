// A rejected token edit must leave the last working credential usable.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testRejectedReplacementKeepsTheSavedTokenThroughCancelAndRestart() throws {
        let app = try connectedSession()
        XCTAssertTrue(app.buttons["header.settings"].waitForExistence(timeout: 15))
        app.buttons["header.settings"].tap()
        app.buttons["settings.connection"].tap()
        let token = app.secureTextFields["connection.token"]
        XCTAssertTrue(token.waitForExistence(timeout: 15))
        token.tap()
        token.typeText(String(repeating: "a", count: 32) + "." + String(repeating: "b", count: 64))
        app.buttons["connection.connect"].tap()
        XCTAssertTrue(app.staticTexts["connection.error"].waitForExistence(timeout: 15))
        capture("Rejected replacement leaves the saved connection editable", screenshot: app.screenshot())
        app.navigationBars["Connection"].buttons["Cancel"].tap()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone exact prompt 🦀")
        XCTAssertTrue(wait { app.buttons["composer.send"].isEnabled }, "Cancel must restore the last working credential")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts["Answer: phone exact prompt 🦀"].waitForExistence(timeout: 15))
        app.terminate()
        app.launchArguments = []
        app.launch()
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone token restart")
        XCTAssertTrue(wait { app.buttons["composer.send"].isEnabled }, "Restart must reuse the last authenticated token")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts["Answer: phone token restart"].waitForExistence(timeout: 15))
        capture("Saved token works after a rejected replacement and a cold restart", screenshot: app.screenshot())
        app.terminate()
    }
}
