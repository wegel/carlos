// Keep XCTest's actual launch, CPU, memory and scroll metrics beside the tested app artifact.
import Foundation
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testFastFlickKeepsNativeMomentum() throws {
        try compareNativeFlick(velocity: .fast)
        try compareNativeFlick(velocity: XCUIGestureVelocity(4000))
    }

    @MainActor
    private func compareNativeFlick(velocity: XCUIGestureVelocity) throws {
        let app = try connectedHost()
        let session = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 15))
        session.tap()
        setTools("Auto", in: app)
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.layout"].buttons["Gutter"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        let transcript = app.collectionViews["transcript"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 15))
        let before = try scrollMetrics(transcript)
        XCTAssertGreaterThan(before.offset, before.height * 3, "This fixture must have plenty of older history")
        fastFlick(in: transcript, velocity: velocity)
        let after = try scrollMetrics(transcript)
        let distance = after.motion - before.motion
        capture("Fast flick travels \(Int(distance)) points across a \(Int(before.height))-point viewport", screenshot: app.screenshot())
        app.terminate()
        app.launchArguments = []
        app.launchEnvironment["CARLOS_NATIVE_SCROLL_REFERENCE"] = "\(before.content)"
        app.launch()
        let reference = app.scrollViews["transcript.native-reference"]
        XCTAssertTrue(reference.waitForExistence(timeout: 15))
        XCTAssertEqual(Double(reference.frame.height), before.height, accuracy: 1)
        let nativeBefore = try scrollMetrics(reference)
        fastFlick(in: reference, velocity: velocity)
        let nativeAfter = try scrollMetrics(reference)
        let nativeDistance = nativeAfter.motion - nativeBefore.motion
        XCTAssertGreaterThan(nativeDistance, before.height * 0.9,
                             "The native control must coast substantially beyond the 0.6-viewport finger path")
        XCTAssertEqual(distance / nativeDistance, 1, accuracy: 0.10,
                       "Carlos must preserve the same fast-flick travel as an ordinary native iOS text panel")
        capture("Carlos flick \(Int(distance)) points, native iOS flick \(Int(nativeDistance)) points", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    private func fastFlick(in element: XCUIElement, velocity: XCUIGestureVelocity) {
        let start = element.coordinate(withNormalizedOffset: .init(dx: 0.5, dy: 0.2))
        let end = element.coordinate(withNormalizedOffset: .init(dx: 0.5, dy: 0.8))
        start.press(forDuration: 0.05, thenDragTo: end, withVelocity: velocity, thenHoldForDuration: 0)
    }

    @MainActor
    private func scrollMetrics(_ element: XCUIElement) throws -> (offset: Double, height: Double, content: Double, motion: Double) {
        let value = try XCTUnwrap(element.value as? String)
        let parts = value.split(separator: "/").compactMap { Double($0) }
        XCTAssertEqual(parts.count, 4)
        return (try XCTUnwrap(parts.first), try XCTUnwrap(parts.dropFirst().first),
                try XCTUnwrap(parts.dropFirst(2).first), try XCTUnwrap(parts.last))
    }

    @MainActor
    func testReadOnlyReferenceScrollsThroughOlderRows() throws {
        let app = try connectedHost()
        let session = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 15))
        session.tap()
        setTools("Auto", in: app)
        let transcript = app.collectionViews["transcript"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 15))
        for layout in ["Flow", "Gutter"] {
            app.buttons["header.settings"].tap()
            app.segmentedControls["settings.layout"].buttons[layout].tap()
            app.navigationBars["Session settings"].buttons["Done"].tap()
            for index in 0..<6 {
                transcript.swipeDown(velocity: .slow)
                capture("Older rows \(layout) \(index)", screenshot: app.screenshot())
            }
            for _ in 0..<4 { transcript.swipeUp(velocity: .fast) }
        }
        app.terminate()
    }

    @MainActor
    func testReadOnlyReferenceScrollPerformance() throws {
        let app = try connectedHost()
        let session = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 15))
        session.tap()
        setTools("Auto", in: app)
        let transcript = app.collectionViews["transcript"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 15))
        let options = XCTMeasureOptions()
        options.iterationCount = 3
        measure(metrics: [XCTOSSignpostMetric.scrollingAndDecelerationMetric,
                          XCTCPUMetric(application: app), XCTMemoryMetric(application: app)], options: options) {
            transcript.swipeDown(velocity: .slow)
            transcript.swipeUp(velocity: .fast)
        }
        capture("Real conversation after measured scrolling", screenshot: app.screenshot())
        app.terminate()
    }

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
        let transcript = app.collectionViews["transcript"]
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
        let transcript = app.collectionViews["transcript"]
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
