// Inspect real native code pixels and copy its untouched Markdown, including Unicode and fences.
import Foundation
import UIKit
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testCodeColorsKeepTheOriginalMarkdownOnTheClipboard() async throws {
        struct Syntax: Decodable { let syntaxSource: String }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let source = try JSONDecoder().decode(Syntax.self, from: Data(contentsOf: file)).syntaxSource
        let app = try connectedSession()
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons["Auto"].tap()
        app.segmentedControls["settings.layout"].buttons["Flow"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone syntax")
        app.buttons["composer.send"].tap()
        let code = app.staticTexts.matching(NSPredicate(format: "identifier == 'code.block' AND label CONTAINS 'fn main()'")).firstMatch
        XCTAssertTrue(wait { code.exists && code.isHittable })
        let colors = try codeColors(app.screenshot(), frame: code.frame, screen: app.frame)
        XCTAssertGreaterThan(colors[0xcba6f7, default: 0], 3, "Rust keywords must paint mauve pixels")
        XCTAssertGreaterThan(colors[0xa6e3a1, default: 0], 3, "Rust strings must paint green pixels")
        XCTAssertGreaterThan(colors[0xfab387, default: 0], 3, "Rust numbers must paint peach pixels")
        XCTAssertTrue(app.staticTexts["keep literal 🦀\n"].exists || app.staticTexts["keep literal 🦀"].exists)
        capture("Actual native code uses the phone palette", screenshot: app.screenshot())
        let copy = app.buttons.matching(NSPredicate(format: "identifier == 'answer.copy' AND value == %@", "\(source.utf8.count) bytes")).firstMatch
        XCTAssertTrue(copy.waitForExistence(timeout: 15))
        let copied = await copiedText(using: copy)
        XCTAssertEqual(copied, source)
        XCTAssertTrue(wait { copy.label == "Copy" })
        app.buttons["header.settings"].tap()
        app.segmentedControls["settings.tools"].buttons["Full"].tap()
        app.navigationBars["Session settings"].buttons["Done"].tap()
        let output = app.scrollViews.matching(NSPredicate(format: "identifier BEGINSWITH 'tool.output.'")).firstMatch
        XCTAssertTrue(wait { output.exists && output.staticTexts.containing(NSPredicate(format: "label CONTAINS 'let count = 42;'")).firstMatch.exists })
        for _ in 0..<3 where output.frame.minY < app.scrollViews["transcript"].frame.minY {
            app.scrollViews["transcript"].swipeDown()
        }
        XCTAssertTrue(output.isHittable)
        let fileText = output.staticTexts.containing(NSPredicate(format: "label CONTAINS 'let count = 42;'")).firstMatch
        XCTAssertLessThanOrEqual(fileText.frame.minX - output.frame.minX, 40,
                                 "Short output must start beside the gutter, without centering inside the box")
        let fileColors = try codeColors(app.screenshot(), frame: output.frame, screen: app.frame)
        XCTAssertGreaterThan(fileColors[0x7f849c, default: 0], 3, "Original native file line numbers must paint in the gutter")
        XCTAssertGreaterThan(fileColors[0xcba6f7, default: 0], 3)
        XCTAssertGreaterThan(fileColors[0xfab387, default: 0], 3)
        capture("Actual file read keeps native line numbers beside colored source", screenshot: app.screenshot())
        app.terminate()
    }

    @MainActor
    private func codeColors(_ screenshot: XCUIScreenshot, frame: CGRect, screen: CGRect) throws -> [UInt32: Int] {
        let image = try XCTUnwrap(screenshot.image.cgImage)
        let scale = CGFloat(image.width) / screen.width
        let crop = try XCTUnwrap(image.cropping(to: CGRect(x: frame.minX * scale, y: frame.minY * scale,
            width: frame.width * scale, height: frame.height * scale).integral))
        var pixels = [UInt8](repeating: 0, count: crop.width * crop.height * 4)
        return try pixels.withUnsafeMutableBytes { bytes in
            let context = try XCTUnwrap(CGContext(data: bytes.baseAddress, width: crop.width, height: crop.height,
                bitsPerComponent: 8, bytesPerRow: crop.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.draw(crop, in: CGRect(x: 0, y: 0, width: crop.width, height: crop.height))
            var found: [UInt32: Int] = [:]
            for index in stride(from: 0, to: bytes.count, by: 4) {
                for color: UInt32 in [0xcba6f7, 0xa6e3a1, 0xfab387, 0x7f849c] {
                    let distance = abs(Int(bytes[index]) - Int((color >> 16) & 255))
                        + abs(Int(bytes[index + 1]) - Int((color >> 8) & 255))
                        + abs(Int(bytes[index + 2]) - Int(color & 255))
                    if distance < 20 { found[color, default: 0] += 1 }
                }
            }
            return found
        }
    }
}
