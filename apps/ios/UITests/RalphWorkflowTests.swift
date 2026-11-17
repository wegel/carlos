// Toggle the actual engine driver and retain an active native turn when Ralph is turned off.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testRalphUsesEngineFactsAndKeepsStopAvailable() throws {
        let app = try connectedSession()
        app.buttons["header.settings"].tap()
        let toggle = app.switches["settings.ralph"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 15))
        XCTAssertEqual(toggle.value as? String, "0")
        toggle.switches.firstMatch.tap()
        XCTAssertTrue(wait { toggle.value as? String == "1" && toggle.isEnabled })
        capture("Engine confirms Ralph on", screenshot: app.screenshot())
        app.navigationBars["Session settings"].buttons["Done"].tap()
        let ralph = app.staticTexts["RALPH"]
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(ralph.waitForExistence(timeout: 15))
        XCTAssertTrue(wait { stop.exists && stop.isEnabled })
        XCTAssertTrue(app.staticTexts["Phone Ralph is working 🌍"].waitForExistence(timeout: 15))
        capture("Ralph activity and original native turn", screenshot: app.screenshot())
        app.buttons["header.settings"].tap()
        XCTAssertTrue(wait { toggle.isEnabled })
        toggle.switches.firstMatch.tap()
        XCTAssertTrue(wait { toggle.value as? String == "0" && toggle.isEnabled })
        app.navigationBars["Session settings"].buttons["Done"].tap()
        XCTAssertTrue(wait { !ralph.exists })
        XCTAssertTrue(stop.isEnabled, "Turning off Ralph must not invent a stopped native turn")
        stop.tap()
        XCTAssertTrue(app.buttons["composer.send"].waitForExistence(timeout: 15))
        XCTAssertFalse(app.buttons["composer.send"].isEnabled)
        capture("Ralph is off and native Stop has completed", screenshot: app.screenshot())
        app.terminate()
    }
}
