// Verify the real unfocused insertion point, including after dismissing the software keyboard.
import UIKit
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func checkIdleComposerCaret(_ app: XCUIApplication, draft: XCUIElement) throws {
        var counts: [Int] = []
        let deadline = Date().addingTimeInterval(4)
        while Date() < deadline {
            let screenshot = app.screenshot()
            let bytes = try rgbaPixels(screenshot, frame: draft.frame, screen: app.frame)
            let count = stride(from: 0, to: bytes.count, by: 4).filter { index in
                Int(bytes[index + 2]) > Int(bytes[index]) + 40
                    && Int(bytes[index + 2]) > Int(bytes[index + 1]) + 40
                    && Int(bytes[index + 1]) > Int(bytes[index]) + 20
            }.count
            if counts.isEmpty || count > (counts.max() ?? 0) || count < (counts.min() ?? 0) {
                capture("Unfocused input caret blue pixels \(count)", screenshot: screenshot)
            }
            counts.append(count)
            if (counts.max() ?? 0) - (counts.min() ?? 0) > 20 { break }
            RunLoop.current.run(until: Date().addingTimeInterval(0.17))
        }
        XCTAssertGreaterThan((counts.max() ?? 0) - (counts.min() ?? 0), 20,
                             "The insertion point must blink without opening the keyboard; samples: \(counts)")
    }
}
