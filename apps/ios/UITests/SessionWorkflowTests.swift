// Drive the actual app over an authenticated forwarded socket into the real deterministic daemon.
import CryptoKit
import Foundation
import UIKit
import XCTest

final class SessionWorkflowTests: XCTestCase {
    @MainActor
    func testRunningToolExpandsStreamsAndKeepsItsRowChoice() throws {
        let app = try connectedSession()
        setTools("Auto", in: app)
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("held")
        app.buttons["composer.send"].tap()
        let tool = app.buttons.matching(NSPredicate(format:
            "identifier BEGINSWITH 'tool.' AND label CONTAINS 'Read log' AND enabled == true")).firstMatch
        XCTAssertTrue(tool.waitForExistence(timeout: 15))
        tool.tap()
        XCTAssertEqual(tool.value as? String, "Expanded")
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'chunk-a'")).firstMatch.waitForExistence(timeout: 15))
        capture("Expanded running tool and offered approval", screenshot: app.screenshot())
        let allow = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'approval.' AND label == 'Allow once'")).firstMatch
        XCTAssertTrue(allow.waitForExistence(timeout: 15))
        allow.tap()
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(wait { stop.isEnabled })
        stop.tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'chunk-b after Stop 🌍'")).firstMatch.waitForExistence(timeout: 15))
        XCTAssertEqual(tool.value as? String, "Expanded", "A native outcome must keep the row's device-local expansion choice")
        tool.tap()
        XCTAssertEqual(tool.value as? String, "Collapsed")
        XCTAssertFalse(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'chunk-b after Stop 🌍'")).firstMatch.exists)
        tool.tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'chunk-b after Stop 🌍'")).firstMatch.waitForExistence(timeout: 15))
        capture("Stopped tool retains complete observed output", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testInvalidTokenKeepsTheFormAndAllowsCorrection() throws {
        let invalid = String(repeating: "a", count: 32) + "." + String(repeating: "b", count: 64)
        let app = try connectedHost(token: invalid)
        XCTAssertTrue(app.staticTexts["connection.error"].waitForExistence(timeout: 15))
        let token = app.secureTextFields["connection.token"]
        XCTAssertTrue(token.exists, "A rejected token must leave the connection form open")
        capture("Rejected token stays editable", screenshot: app.screenshot())
        token.tap()
        token.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: invalid.count))
        token.typeText(try settings().token)
        app.buttons["connection.connect"].tap()
        XCTAssertTrue(app.textFields["sessions.cwd"].waitForExistence(timeout: 15))
        XCTAssertFalse(token.exists)
        capture("Authenticated host opens sessions", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testRelaunchReusesSavedConnectionAndToken() throws {
        let app = try connectedSession()
        XCTAssertTrue(app.buttons["header.sessions"].waitForExistence(timeout: 15))
        XCTAssertTrue(wait { app.buttons["header.title"].label.contains("codex") })
        let title = app.buttons["header.title"].label
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.textViews["composer.draft"].waitForExistence(timeout: 15))
        XCTAssertFalse(app.secureTextFields["connection.token"].exists)
        app.terminate()
        app.launchArguments = []
        app.launch()
        let sessions = app.buttons["header.sessions"]
        XCTAssertTrue(sessions.waitForExistence(timeout: 15), "Relaunch must restore the saved host")
        XCTAssertFalse(app.secureTextFields["connection.token"].exists)
        XCTAssertTrue(wait { app.buttons["header.title"].label == title }, "Relaunch must reopen the last session")
        sessions.tap()
        let saved = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(saved.waitForExistence(timeout: 15))
        XCTAssertTrue(wait { saved.isEnabled }, "The saved token must authenticate without re-entry")
        saved.tap()
        app.buttons["header.settings"].tap()
        app.buttons["settings.connection"].tap()
        XCTAssertTrue(app.secureTextFields["connection.token"].waitForExistence(timeout: 15))
        let reconnect = app.buttons["connection.connect"]
        XCTAssertTrue(reconnect.isEnabled, "An existing connection must reuse its saved token when the field is empty")
        let address = app.textFields["connection.address"]
        address.tap()
        address.typeText("/other-host")
        XCTAssertFalse(reconnect.isEnabled, "A saved token must never be sent to an edited address implicitly")
        address.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 11))
        XCTAssertTrue(reconnect.isEnabled)
        capture("Saved connection reuses Keychain token", screenshot: app.screenshot())
        reconnect.tap()
        XCTAssertTrue(wait { !app.secureTextFields["connection.token"].exists })
        app.terminate()
    }

    @MainActor
    func testReadOnlyReferenceKeepsFixedControlsInBothLayouts() throws {
        let app = try connectedHost()
        let session = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 15))
        session.tap()
        setTools("Auto", in: app)
        var headerPixels: Data?
        var composerPixels: Data?
        for layout in ["Flow", "Gutter"] {
            let settings = app.buttons["header.settings"]
            XCTAssertTrue(settings.waitForExistence(timeout: 15))
            settings.tap()
            let picker = app.segmentedControls["settings.layout"]
            XCTAssertTrue(picker.waitForExistence(timeout: 15))
            picker.buttons[layout].tap()
            app.navigationBars["Session settings"].buttons["Done"].tap()
            let header = app.buttons["header.sessions"]
            let draft = app.textViews["composer.draft"]
            XCTAssertTrue(wait { header.isHittable && draft.isHittable })
            let transcript = app.collectionViews["transcript"]
            XCTAssertGreaterThanOrEqual(transcript.frame.minY, header.frame.maxY)
            XCTAssertGreaterThanOrEqual(draft.frame.minY, transcript.frame.maxY)
            XCTAssertGreaterThan(transcript.staticTexts.count, 0)
            let screenshot = app.screenshot()
            let title = try pixels(screenshot, frame: app.buttons["header.title"].frame, screen: app.frame)
            // The idle insertion point blinks independently of the transcript layout.
            let input = try pixels(screenshot, frame: draft.frame.insetBy(dx: 4, dy: 0), screen: app.frame)
            XCTAssertEqual(draft.value as? String, "")
            if let headerPixels { XCTAssertEqual(title, headerPixels, "Changing the transcript layout must keep the title painted") }
            if let composerPixels { XCTAssertEqual(input, composerPixels, "Changing the transcript layout must keep the draft painted") }
            headerPixels = title
            composerPixels = input
            let screen = XCTAttachment(screenshot: screenshot)
            screen.name = "Read-only reference \(layout)"
            screen.lifetime = .keepAlways
            add(screen)
            try checkReaderSettles(in: transcript)
            capture("Older reference stays put in \(layout)", screenshot: app.screenshot())
        }
        app.terminate()
    }

    @MainActor
    private func pixels(_ screenshot: XCUIScreenshot, frame: CGRect, screen: CGRect) throws -> Data {
        let image = try XCTUnwrap(screenshot.image.cgImage)
        let x = CGFloat(image.width) / screen.width, y = CGFloat(image.height) / screen.height
        let region = CGRect(x: frame.minX * x, y: frame.minY * y, width: frame.width * x, height: frame.height * y).integral
        let cropped = try XCTUnwrap(image.cropping(to: region))
        return try XCTUnwrap(UIImage(cgImage: cropped).pngData())
    }

    @MainActor
    func testConnectCreateAndSendOneExactPrompt() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone exact prompt 🦀")
        let send = app.buttons["composer.send"]
        XCTAssertTrue(wait { send.isEnabled })
        send.tap()
        XCTAssertTrue(app.staticTexts["Answer: phone exact prompt 🦀"].waitForExistence(timeout: 15))
        XCTAssertEqual(draft.value as? String, "")
        capture("Actual daemon answer", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testApprovalAndStopPassAHeldPromptReply() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("held")
        let send = app.buttons["composer.send"]
        XCTAssertTrue(wait { send.isEnabled })
        send.tap()
        let allow = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'approval.' AND label == 'Allow once'")).firstMatch
        XCTAssertTrue(allow.waitForExistence(timeout: 15))
        XCTAssertTrue(wait { allow.isEnabled })
        allow.tap()
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(stop.waitForExistence(timeout: 15))
        XCTAssertTrue(wait { stop.isEnabled })
        stop.tap()
        // The fixture emits a real native completion after receiving the original interrupt.
        XCTAssertTrue(app.buttons["composer.send"].waitForExistence(timeout: 15))
        XCTAssertFalse(app.buttons["composer.send"].isEnabled)
        capture("Native held approval and Stop completed", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testLongOriginalSourceAndCopyKeepEveryByte() async throws {
        let fixture = try settings()
        let expected = try XCTUnwrap(fixture.sourceSha256)
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone long source")
        let send = app.buttons["composer.send"]
        XCTAssertTrue(wait { send.isEnabled })
        send.tap()
        guard app.staticTexts["phone source complete 🦀"].waitForExistence(timeout: 60) else {
            XCTFail("The complete source's last paragraph did not appear")
            return
        }
        let count = try XCTUnwrap(fixture.sourceBytes)
        // Match the complete source by its announced byte count, never a historical answer's Copy.
        let copy = app.buttons.matching(NSPredicate(format: "identifier == 'answer.copy' AND value == %@", "\(count) bytes")).firstMatch
        guard copy.waitForExistence(timeout: 15), copy.isHittable else {
            XCTFail("The complete source's Copy button is not visible")
            return
        }
        let copied = await copiedText(using: copy)
        let bytes = Data(try XCTUnwrap(copied).utf8)
        XCTAssertEqual(bytes.count, fixture.sourceBytes)
        let actual = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
        XCTAssertEqual(actual, expected)
        XCTAssertTrue(wait { copy.label == "Copy" })
        capture("Complete original source and copied bytes", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func capture(_ name: String, screenshot: XCUIScreenshot) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    @MainActor
    func connectedSession(hostName: String = "Native fixture") throws -> XCUIApplication {
        let fixture = try settings()
        let app = try connectedHost(name: hostName)
        let cwd = app.textFields["sessions.cwd"]
        XCTAssertTrue(cwd.waitForExistence(timeout: 15))
        cwd.tap()
        cwd.typeText(fixture.cwd)
        if let backend = fixture.backend, backend != "codex" {
            app.segmentedControls["sessions.backend"].buttons[backend].tap()
        }
        let start = app.buttons["sessions.start"]
        XCTAssertTrue(wait { start.isEnabled })
        start.tap()
        return app
    }

    @MainActor
    func connectedHost(token suppliedToken: String? = nil, name: String = "Native fixture") throws -> XCUIApplication {
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
        host.typeText(name)
        let token = app.secureTextFields["connection.token"]
        token.tap()
        token.typeText(suppliedToken ?? fixture.token)
        let privateSwitch = app.switches["connection.private"]
        privateSwitch.switches.firstMatch.tap()
        XCTAssertEqual(privateSwitch.value as? String, "1")
        let connect = app.buttons["connection.connect"]
        XCTAssertTrue(connect.isHittable)
        if app.keyboards.firstMatch.exists {
            XCTAssertLessThanOrEqual(connect.frame.maxY, app.keyboards.firstMatch.frame.minY,
                                     "The connection action must stay above the keyboard")
            let done = app.buttons["connection.keyboardDone"]
            XCTAssertTrue(done.waitForExistence(timeout: 5))
            XCTAssertLessThanOrEqual(done.frame.maxY, connect.frame.minY)
            capture("Connection action stays clear of the keyboard and Done", screenshot: app.screenshot())
        }
        connect.tap()
        return app
    }

    private func settings() throws -> Fixture {
        guard let url = Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json") else {
            throw XCTSkip("Use scripts/ios-fixture.py to provision the isolated daemon")
        }
        return try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }

    @MainActor
    func wait(_ predicate: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(15)
        while Date() < deadline {
            if predicate() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        }
        return false
    }
}

private struct Fixture: Decodable {
    let backend: String?
    let url: String
    let token: String
    let cwd: String
    let sourceSha256: String?
    let sourceBytes: Int?
}
