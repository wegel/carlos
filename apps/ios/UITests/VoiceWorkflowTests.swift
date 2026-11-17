// Exercise the real platform's unavailable or denied voice-input path without recording or sending.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testUnavailableOrDeniedVoiceInputKeepsThePrivateDraft() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("private unsent voice draft 🦀")
        app.buttons["composer.microphone"].tap()
        let system = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        XCTAssertTrue(wait { app.staticTexts["voice.error"].exists || system.alerts.firstMatch.exists })
        if !app.staticTexts["voice.error"].exists {
            let deny = system.alerts.buttons["Don’t Allow"].exists ? system.alerts.buttons["Don’t Allow"] : system.alerts.buttons["Don't Allow"]
            XCTAssertTrue(deny.exists, "Only deny access; this test must never start a recording")
            deny.tap()
        }
        let error = app.staticTexts["voice.error"]
        XCTAssertTrue(error.waitForExistence(timeout: 15))
        XCTAssertTrue(error.label.contains("unavailable") || error.label.contains("Settings"))
        capture("Voice input gives an honest native availability or permission message", screenshot: app.screenshot())
        app.buttons["voice.dismiss"].tap()
        XCTAssertEqual(draft.value as? String, "private unsent voice draft 🦀")
        XCTAssertTrue(app.buttons["composer.send"].isEnabled)
        app.terminate()
    }
}
