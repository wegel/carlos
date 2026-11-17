// Exercise saved-server edits/removal through the real form, Keychain and daemon.
import Foundation
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testEditedServerKeepsItsEntryThroughRestartAndRemoval() throws {
        let fixture = try hostFixture()
        let app = try connectedSession(hostName: "Alpha")
        XCTAssertTrue(app.buttons["header.sessions"].waitForExistence(timeout: 15))
        app.buttons["header.sessions"].tap()
        let manage = hostMenus(app).firstMatch
        XCTAssertTrue(manage.waitForExistence(timeout: 15))
        let identifier = manage.identifier
        manage.tap()
        app.buttons["Edit connection"].tap()
        try replaceHostField(app.textFields["connection.host"], with: "Daily Alpha")
        try replaceHostField(app.textFields["connection.address"], with: fixture.hostAliasURL)
        app.secureTextFields["connection.token"].tap()
        app.secureTextFields["connection.token"].typeText(fixture.token)
        app.buttons["connection.connect"].tap()
        XCTAssertTrue(app.buttons[identifier].waitForExistence(timeout: 15))
        XCTAssertEqual(hostMenus(app).count, 1, "Editing an address must replace its entry, not add another Alpha")
        XCTAssertTrue(app.staticTexts[fixture.hostAliasURL].exists)
        capture("Edited server keeps its identity and shows its address", screenshot: app.screenshot())
        let session = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'session.'")).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 15))
        session.tap()
        XCTAssertTrue(app.textViews["composer.draft"].waitForExistence(timeout: 15))
        app.terminate()
        app.launchArguments = []
        app.launch()
        XCTAssertTrue(app.buttons["header.sessions"].waitForExistence(timeout: 15))
        app.buttons["header.sessions"].tap()
        XCTAssertTrue(app.buttons[identifier].waitForExistence(timeout: 15))
        XCTAssertEqual(hostMenus(app).count, 1)
        app.buttons[identifier].tap()
        app.buttons["Remove from iPhone"].tap()
        XCTAssertTrue(app.alerts.firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue(app.alerts.firstMatch.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", fixture.hostAliasURL)).firstMatch.exists)
        app.alerts.firstMatch.buttons["Cancel"].tap()
        XCTAssertEqual(hostMenus(app).count, 1)
        app.buttons[identifier].tap()
        app.buttons["Remove from iPhone"].tap()
        app.alerts.firstMatch.buttons["host.confirmRemove"].firstMatch.tap()
        XCTAssertTrue(app.textFields["connection.address"].waitForExistence(timeout: 15))
        capture("Removing the last server returns to a blank connection form", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    func testSameNamedServersShowAddressesAndRemoveOnlyTheChosenEntry() throws {
        let fixture = try hostFixture()
        let app = try connectedSession(hostName: "Alpha")
        XCTAssertTrue(app.buttons["header.sessions"].waitForExistence(timeout: 15))
        app.buttons["header.sessions"].tap()
        let first = hostMenus(app).firstMatch
        XCTAssertTrue(first.waitForExistence(timeout: 15))
        let firstID = first.identifier
        app.buttons["sessions.addHost"].tap()
        XCTAssertTrue(app.textFields["connection.address"].waitForExistence(timeout: 15))
        app.textFields["connection.host"].tap()
        app.textFields["connection.host"].typeText("Alpha")
        app.textFields["connection.address"].tap()
        app.textFields["connection.address"].typeText(fixture.hostAliasURL)
        app.secureTextFields["connection.token"].tap()
        app.secureTextFields["connection.token"].typeText(fixture.token)
        let toggle = app.switches["connection.private"]
        toggle.switches.firstMatch.tap()
        app.buttons["connection.connect"].tap()
        XCTAssertTrue(wait { hostMenus(app).count == 2 })
        XCTAssertTrue(app.staticTexts[fixture.url].exists)
        XCTAssertTrue(app.staticTexts[fixture.hostAliasURL].exists)
        capture("Two Alpha entries have distinct addresses and visible Manage buttons", screenshot: app.screenshot())
        app.buttons[firstID].tap()
        app.buttons["Remove from iPhone"].tap()
        app.alerts.firstMatch.buttons["host.confirmRemove"].firstMatch.tap()
        XCTAssertTrue(wait { hostMenus(app).count == 1 })
        XCTAssertFalse(app.buttons[firstID].exists)
        XCTAssertTrue(app.staticTexts[fixture.hostAliasURL].exists)
        app.terminate()
        app.launchArguments = []
        app.launch()
        XCTAssertTrue(app.buttons["header.sessions"].waitForExistence(timeout: 15))
        app.buttons["header.sessions"].tap()
        XCTAssertTrue(wait { hostMenus(app).count == 1 })
        XCTAssertTrue(app.staticTexts[fixture.hostAliasURL].exists)
        capture("Only the chosen saved server was removed, including after relaunch", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    private func hostMenus(_ app: XCUIApplication) -> XCUIElementQuery {
        app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'host.manage.'"))
    }

    @MainActor
    private func replaceHostField(_ field: XCUIElement, with value: String) throws {
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        let previous = try XCTUnwrap(field.value as? String)
        field.coordinate(withNormalizedOffset: .init(dx: 0.95, dy: 0.5)).tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: previous.count) + value)
        XCTAssertEqual(field.value as? String, value)
    }

    private func hostFixture() throws -> (url: String, token: String, hostAliasURL: String) {
        struct Settings: Decodable { let url: String; let token: String; let hostAliasURL: String }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let values = try JSONDecoder().decode(Settings.self, from: Data(contentsOf: file))
        return (values.url, values.token, values.hostAliasURL)
    }
}
