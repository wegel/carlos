// Inspect actual painted caret pixels while one native turn waits for Stop.
import UIKit
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testStreamingCaretBlinksAndStopsWithTheOriginalTurn() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone stream")
        app.buttons["composer.send"].tap()
        let streaming = app.staticTexts["answer.streaming"]
        XCTAssertTrue(streaming.waitForExistence(timeout: 15))
        XCTAssertEqual(streaming.label, "Streaming paragraph 🌍", "The caret must not become source or accessibility text")
        let prompt = app.staticTexts["phone stream"]
        XCTAssertTrue(prompt.exists)
        XCTAssertLessThan(prompt.frame.maxY, streaming.frame.minY, "A private prompt echo must precede the answer it started")
        try checkCaretBlink(app, text: streaming)
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(wait { stop.isEnabled })
        stop.tap()
        XCTAssertTrue(app.buttons["composer.send"].waitForExistence(timeout: 15))
        XCTAssertFalse(streaming.exists)
        XCTAssertTrue(app.staticTexts["Streaming paragraph 🌍"].exists)
        let bytes = Data("Streaming paragraph 🌍".utf8).count
        let copy = app.buttons.matching(NSPredicate(format: "identifier == 'answer.copy' AND value == %@", "\(bytes) bytes")).firstMatch
        XCTAssertTrue(copy.waitForExistence(timeout: 15))
        capture("Stopped native answer keeps text and Copy without a caret", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    private func checkCaretBlink(_ app: XCUIApplication, text: XCUIElement) throws {
        var counts: [Int] = []
        let deadline = Date().addingTimeInterval(4)
        while Date() < deadline {
            let screen = app.screenshot()
            let count = try greenPixels(screen, frame: text.frame.insetBy(dx: -10, dy: -2), screen: app.frame)
            if counts.isEmpty || count > (counts.max() ?? 0) || count < (counts.min() ?? 0) {
                capture("Native streaming caret green pixels \(count)", screenshot: screen)
            }
            counts.append(count)
            if (counts.max() ?? 0) - (counts.min() ?? 0) > 20 { break }
            RunLoop.current.run(until: Date().addingTimeInterval(0.17))
        }
        XCTAssertGreaterThan((counts.max() ?? 0) - (counts.min() ?? 0), 20, "The actual caret must blink; samples: \(counts)")
    }

    @MainActor
    private func greenPixels(_ screenshot: XCUIScreenshot, frame: CGRect, screen: CGRect) throws -> Int {
        let nativeImage = screenshot.image
        let image = try XCTUnwrap(nativeImage.cgImage)
        let x = CGFloat(image.width) / screen.width, y = CGFloat(image.height) / screen.height
        let rectangle = CGRect(x: frame.minX * x, y: frame.minY * y, width: frame.width * x, height: frame.height * y).integral
        let crop = try XCTUnwrap(image.cropping(to: rectangle))
        var bytes = [UInt8](repeating: 0, count: crop.width * crop.height * 4)
        try bytes.withUnsafeMutableBytes { buffer in
            let context = try XCTUnwrap(CGContext(data: buffer.baseAddress, width: crop.width, height: crop.height,
                bitsPerComponent: 8, bytesPerRow: crop.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue))
            context.draw(crop, in: CGRect(x: 0, y: 0, width: crop.width, height: crop.height))
        }
        return stride(from: 0, to: bytes.count, by: 4).filter { index in
            Int(bytes[index + 1]) > Int(bytes[index]) + 20 && Int(bytes[index + 1]) > Int(bytes[index + 2]) + 20
        }.count
    }
}
