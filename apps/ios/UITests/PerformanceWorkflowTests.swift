// Keep XCTest's actual launch, CPU, memory and scroll metrics beside the tested app artifact.
import Foundation
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testIdleCpuAndResidentMemory() async throws {
        struct Idle: Decodable { let idleControlURL: URL }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let control = try JSONDecoder().decode(Idle.self, from: Data(contentsOf: file))
        let app = try connectedSession()
        XCTAssertTrue(app.textViews["composer.draft"].waitForExistence(timeout: 15))
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons["Auto"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        var request = URLRequest(url: control.idleControlURL)
        request.httpMethod = "POST"
        let (_, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertTrue(app.textViews["composer.draft"].isHittable)
        capture("Idle native transcript and fixed controls", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testSavedLaunchPerformance() throws {
        let app = try connectedSession()
        XCTAssertTrue(app.textViews["composer.draft"].waitForExistence(timeout: 15))
        app.terminate()
        app.launchArguments = []
        let options = XCTMeasureOptions()
        options.iterationCount = 3
        measure(metrics: [XCTApplicationLaunchMetric(waitUntilResponsive: true)], options: options) {
            app.launch()
            XCTAssertTrue(app.textViews["composer.draft"].waitForExistence(timeout: 15))
            XCTAssertFalse(app.secureTextFields["connection.token"].exists)
            app.terminate()
        }
    }

    @MainActor
    func testPrivateTypingAndNativeScrollPerformance() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        let options = XCTMeasureOptions()
        options.iterationCount = 3
        measure(metrics: [XCTClockMetric(), XCTCPUMetric(application: app), XCTMemoryMetric(application: app)], options: options) {
            draft.typeText("private typing 🌍")
            XCTAssertEqual(draft.value as? String, "private typing 🌍")
            draft.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 16))
            XCTAssertEqual(draft.value as? String, "")
        }
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons["Auto"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        let transcript = app.scrollViews["transcript"]
        XCTAssertGreaterThan(transcript.staticTexts.count, 0)
        transcript.swipeDown(velocity: .slow)
        transcript.swipeUp(velocity: .slow)
        capture("Native transcript remains readable after measured private typing", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testNativeScrollPerformance() throws {
        let app = try connectedSession()
        XCTAssertTrue(app.textViews["composer.draft"].waitForExistence(timeout: 15))
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons["Auto"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        let transcript = app.scrollViews["transcript"]
        XCTAssertGreaterThan(transcript.staticTexts.count, 0)
        let options = XCTMeasureOptions()
        options.iterationCount = 3
        measure(metrics: [XCTOSSignpostMetric.scrollingAndDecelerationMetric,
                          XCTCPUMetric(application: app), XCTMemoryMetric(application: app)], options: options) {
            transcript.swipeDown(velocity: .slow)
            transcript.swipeUp(velocity: .slow)
        }
        capture("Measured native scrolling keeps the fixed controls painted", screenshot: app.screenshot())
        app.terminate()
    }
}
