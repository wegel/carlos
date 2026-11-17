// Measure actual screen positions through expansion, manual scrolling and native completion.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testToolExpansionAndNativeUpdatesKeepTheReadersPlace() throws {
        let app = try connectedSession()
        setTools("Auto", in: app)
        let transcript = app.scrollViews["transcript"]
        let tool = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'tool.' AND label CONTAINS 'history.txt'")).firstMatch
        XCTAssertTrue(wait { tool.exists && tool.isHittable })
        let originalY = tool.frame.minY
        let originalX = tool.frame.minX
        tool.tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'anchor-output 39'")).firstMatch.waitForExistence(timeout: 15))
        XCTAssertEqual(tool.frame.minY, originalY, accuracy: 2, "Expanding a tool must keep its header in place")
        XCTAssertEqual(tool.frame.minX, originalX, accuracy: 2, "Expanding must preserve the horizontal margin too")
        capture("Expanded tool keeps the original header position", screenshot: app.screenshot())
        tool.tap()
        XCTAssertEqual(tool.frame.minY, originalY, accuracy: 2, "Collapsing a tool must keep its header in place")
        let draft = app.textViews["composer.draft"]
        draft.tap()
        draft.typeText("phone stream")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts["answer.streaming"].waitForExistence(timeout: 15))
        transcript.swipeDown(velocity: .slow)
        transcript.swipeDown(velocity: .slow)
        let latest = app.buttons["transcript.latest"]
        XCTAssertTrue(latest.waitForExistence(timeout: 15))
        let history = try XCTUnwrap(transcript.staticTexts.allElementsBoundByIndex.first {
            $0.label.hasPrefix("history ") && $0.frame.minY > transcript.frame.minY + 10 && $0.frame.maxY < transcript.frame.maxY - 10
        })
        let readingY = history.frame.minY
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(wait { stop.isEnabled })
        stop.tap()
        XCTAssertTrue(app.buttons["composer.send"].waitForExistence(timeout: 15))
        XCTAssertEqual(history.frame.minY, readingY, accuracy: 2, "Native completion must not pull a reader away from older text")
        XCTAssertTrue(latest.exists)
        capture("Native completion keeps the older reading position", screenshot: app.screenshot())
        latest.tap()
        XCTAssertTrue(wait { app.staticTexts["Streaming paragraph 🌍"].isHittable })
        XCTAssertTrue(wait { !latest.exists })
        capture("Latest returns to the completed answer", screenshot: app.screenshot())
        app.terminate()
    }
}
