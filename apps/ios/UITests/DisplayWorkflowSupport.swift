// Choose the real saved display mode before testing a workflow that depends on its starting state.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func setTools(_ mode: String, in app: XCUIApplication) {
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons[mode].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
    }
}
