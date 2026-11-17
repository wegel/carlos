// Read Claude's actual native weekly allowance beside the current context percentage.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testClaudeWeeklyQuotaSitsBesideContext() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("native quotas")
        app.buttons["composer.send"].tap()
        let quota = app.staticTexts["session.weeklyQuota"]
        XCTAssertTrue(quota.waitForExistence(timeout: 15))
        XCTAssertTrue(quota.label.hasPrefix("W60%/1.0d"), quota.label)
        let context = app.staticTexts["session.contextPercent"]
        XCTAssertTrue(context.exists)
        XCTAssertGreaterThanOrEqual(quota.frame.minX, context.frame.maxX)
        XCTAssertLessThanOrEqual(quota.frame.minX - context.frame.maxX, 12)
        XCTAssertEqual(quota.frame.midY, context.frame.midY, accuracy: 2)
        draft.tap()
        draft.typeText("private quota draft 🌍")
        XCTAssertEqual(draft.value as? String, "private quota draft 🌍")
        capture("Claude weekly quota beside context", screenshot: app.screenshot())
        app.terminate()
    }
}
