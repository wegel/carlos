// Drive the actual app over an authenticated forwarded socket into the real deterministic daemon.
import Foundation
import XCTest

final class SessionWorkflowTests: XCTestCase {
    @MainActor
    func testConnectCreateAndSendOneExactPrompt() throws {
        continueAfterFailure = false
        let fixture = try settings()
        let app = XCUIApplication(bundleIdentifier: "app.carlos.phone.fixture")
        app.launchArguments = ["-carlos.hosts.v1", ""]
        app.launch()
        let address = app.textFields["connection.address"]
        XCTAssertTrue(address.waitForExistence(timeout: 15))
        address.tap()
        address.typeText(fixture.url)
        let host = app.textFields["connection.host"]
        host.tap()
        host.typeText("Native fixture")
        let token = app.secureTextFields["connection.token"]
        token.tap()
        token.typeText(fixture.token)
        let privateSwitch = app.switches["connection.private"]
        privateSwitch.switches.firstMatch.tap()
        XCTAssertEqual(privateSwitch.value as? String, "1")
        app.buttons["connection.connect"].tap()
        let cwd = app.textFields["sessions.cwd"]
        XCTAssertTrue(cwd.waitForExistence(timeout: 15))
        cwd.tap()
        cwd.typeText(fixture.cwd)
        let start = app.buttons["sessions.start"]
        XCTAssertTrue(wait { start.isEnabled })
        start.tap()
        let draft = app.textFields["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone exact prompt 🦀")
        let send = app.buttons["composer.send"]
        XCTAssertTrue(wait { send.isEnabled })
        send.tap()
        XCTAssertTrue(app.staticTexts["Answer: phone exact prompt 🦀"].waitForExistence(timeout: 15))
        XCTAssertEqual(draft.value as? String, "")
        let screen = XCTAttachment(screenshot: app.screenshot())
        screen.name = "Actual daemon answer"
        screen.lifetime = .keepAlways
        add(screen)
        app.terminate()
    }

    private func settings() throws -> Fixture {
        guard let url = Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json") else {
            throw XCTSkip("Use scripts/ios-fixture.py to provision the isolated daemon")
        }
        return try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }

    @MainActor
    private func wait(_ predicate: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(15)
        while Date() < deadline {
            if predicate() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        }
        return false
    }
}

private struct Fixture: Decodable {
    let url: String
    let token: String
    let cwd: String
}
