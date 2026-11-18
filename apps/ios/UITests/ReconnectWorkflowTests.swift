// Preserve submitted uncertainty and a newer private draft across a real daemon restart.
import Foundation
import UIKit
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testRestartRetainsPrivateTextWithoutResendingAndReopensTheSession() async throws {
        struct Restart: Decodable { let reconnectControlURL: URL }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let control = try JSONDecoder().decode(Restart.self, from: Data(contentsOf: file))
        let submitted = "phone reconnect\nexact 🦀"
        let privateText = "phone private draft 🌍"
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText(submitted)
        XCTAssertEqual(draft.value as? String, submitted)
        app.buttons["composer.send"].tap()
        XCTAssertTrue(wait { draft.value as? String == "" })
        draft.typeText(privateText)
        XCTAssertEqual(draft.value as? String, privateText)
        var request = URLRequest(url: control.reconnectControlURL)
        request.httpMethod = "POST"
        let (_, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertTrue(app.staticTexts["session.error"].waitForExistence(timeout: 15))
        let reconnect = app.buttons["session.reconnect"]
        XCTAssertTrue(reconnect.waitForExistence(timeout: 15))
        reconnect.tap()
        XCTAssertTrue(wait { !reconnect.exists })
        XCTAssertFalse(app.secureTextFields["connection.token"].exists)
        XCTAssertEqual(draft.value as? String, privateText)
        XCTAssertTrue(app.staticTexts["Submitted text retained"].waitForExistence(timeout: 15))
        XCTAssertEqual(app.staticTexts.matching(NSPredicate(format: "label == %@", submitted)).count, 1,
                       "An uncertain prompt belongs in its recovery card, without a duplicate private user band")
        let restore = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'recovery.restore.'")).firstMatch
        XCTAssertTrue(restore.exists)
        XCTAssertFalse(restore.isEnabled, "The retained prompt must not replace a newer private draft")
        capture("A real daemon restart retains both private texts without resending", screenshot: app.screenshot())
        let reopen = app.buttons["Reopen session"]
        XCTAssertTrue(reopen.waitForExistence(timeout: 15))
        reopen.tap()
        XCTAssertTrue(wait { app.buttons["composer.send"].isEnabled })
        XCTAssertEqual(draft.value as? String, privateText)
        let copy = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'recovery.copy.'")).firstMatch
        XCTAssertTrue(copy.isHittable)
        let copied = await copiedText(using: copy)
        XCTAssertEqual(copied, submitted)
        XCTAssertEqual(draft.value as? String, privateText)
        capture("The reopened session keeps the newer draft and recoverable original text", screenshot: app.screenshot())
        app.terminate()
    }
}
