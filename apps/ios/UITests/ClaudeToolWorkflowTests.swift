// Keep separate native sources available while folding a proven Claude call/result pair.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testClaudeCallAndResultShareOneCompactHeader() throws {
        let app = try connectedSession()
        setTools("Auto", in: app)
        let draft = app.textViews["composer.draft"]
        draft.tap()
        draft.typeText("read source")
        app.buttons["composer.send"].tap()
        let tools = app.buttons.matching(NSPredicate(format:
            "identifier BEGINSWITH 'tool.' AND label CONTAINS 'source.rs'"))
        XCTAssertTrue(wait { tools.count == 1 && tools.firstMatch.isEnabled })
        tools.firstMatch.tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format:
            "label CONTAINS 'let claude_read_7 = 7;'")).firstMatch.waitForExistence(timeout: 15))
        capture("One Claude execution with complete output", screenshot: app.screenshot())
        setTools("Full", in: app)
        XCTAssertTrue(wait { tools.count == 2 }, "Full view must retain both original source rows")
        capture("Full view retains original Claude call and result", screenshot: app.screenshot())
        app.terminate()
    }
}
