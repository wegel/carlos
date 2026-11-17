// Choose the real saved display mode before testing a workflow that depends on its starting state.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func setTools(_ mode: String, in app: XCUIApplication) {
        XCTAssertTrue(wait { !app.buttons["sessions.start"].exists && app.buttons["header.settings"].isHittable })
        app.buttons["header.settings"].tap()
        let tools = app.segmentedControls["settings.tools"]
        XCTAssertTrue(tools.waitForExistence(timeout: 15))
        tools.buttons[mode].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
    }

    @MainActor
    func checkReaderSettles(in transcript: XCUIElement) throws {
        transcript.swipeDown(velocity: .slow)
        let cell = try XCTUnwrap(transcript.cells.allElementsBoundByIndex.first {
            $0.identifier.hasPrefix("entry.") && $0.frame.maxY > transcript.frame.minY + 10
                && $0.frame.minY < transcript.frame.maxY - 10
        })
        let visible = transcript.cells[cell.identifier]
        let frame = visible.frame
        for _ in 0..<4 {
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
            XCTAssertEqual(visible.frame.minY, frame.minY, accuracy: 2,
                           "Older text must stay put after a scroll ends")
            XCTAssertEqual(visible.frame.minX, frame.minX, accuracy: 2)
        }
    }
}
