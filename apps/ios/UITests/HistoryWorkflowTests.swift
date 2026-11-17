// Fetch the real older native rows and measure the original on-screen anchor after prepending.
import Foundation
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testReachingTheTopLoadsOlderHistoryWithoutMovingText() async throws {
        let app = try connectedHost()
        let saved = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(saved.waitForExistence(timeout: 15))
        saved.tap()
        let transcript = app.scrollViews["transcript"]
        let last = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH 'history anchor 129 🌍'")).firstMatch
        XCTAssertTrue(wait { last.exists && last.isHittable })
        let earlier = app.buttons["transcript.history"]
        let anchor = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH 'history anchor 30 🌍'")).firstMatch
        // Drag down until the original first native row reaches the top of the viewport.
        for _ in 0..<5 {
            if earlier.exists && !earlier.isEnabled { break }
            transcript.swipeDown(velocity: .slow)
        }
        XCTAssertTrue(wait { anchor.exists && anchor.isHittable })
        XCTAssertTrue(wait { earlier.exists && !earlier.isEnabled }, "Reaching the top must start the real native history read")
        let readingY = anchor.frame.minY
        let readingX = anchor.frame.minX
        try await releaseHistory()
        XCTAssertTrue(wait { earlier.isEnabled })
        XCTAssertEqual(anchor.frame.minY, readingY, accuracy: 2, "A failed read must keep the original text in place")
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'native history fixture read failed'")).firstMatch.exists)
        capture("A failed native page keeps the reader's position and offers retry", screenshot: app.screenshot())
        earlier.tap()
        XCTAssertTrue(wait { !earlier.isEnabled })
        try await releaseHistory()
        XCTAssertTrue(wait { !earlier.exists }, "Reaching the top must load the remaining older native rows")
        XCTAssertEqual(anchor.frame.minY, readingY, accuracy: 2, "Prepending must preserve the original text's exact vertical position")
        XCTAssertEqual(anchor.frame.minX, readingX, accuracy: 2, "Prepending must preserve the original horizontal margin")
        capture("Earlier native rows load above the original reading position", screenshot: app.screenshot())
        transcript.swipeDown(velocity: .slow)
        let oldest = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH 'history anchor 0 🌍'")).firstMatch
        XCTAssertTrue(wait { oldest.exists && oldest.isHittable })
        XCTAssertFalse(earlier.exists)
        XCTAssertEqual(app.textViews["composer.draft"].value as? String, "")
        capture("The oldest native message is reachable", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor private func releaseHistory() async throws {
        struct History: Decodable { let historyControlURL: URL }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let control = try JSONDecoder().decode(History.self, from: Data(contentsOf: file))
        var request = URLRequest(url: control.historyControlURL)
        request.httpMethod = "POST"
        let (_, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200, "The original native history read must be held before release")
    }
}
