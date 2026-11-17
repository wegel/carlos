// Keep a ten-thousand-line native code fence responsive and copy the complete original Markdown.
import CryptoKit
import Foundation
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testLargeCodeFenceKeepsEveryOriginalByteAndStaysResponsive() async throws {
        struct Source: Decodable { let sourceSha256: String; let sourceBytes: Int }
        let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "fixture", withExtension: "json"))
        let source = try JSONDecoder().decode(Source.self, from: Data(contentsOf: file))
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone large code")
        app.buttons["composer.send"].tap()
        XCTAssertTrue(app.staticTexts["phone source complete 🦀"].waitForExistence(timeout: 60))
        let code = app.scrollViews["code.viewport"].firstMatch
        XCTAssertTrue(code.exists)
        XCTAssertLessThanOrEqual(code.frame.height, 320)
        XCTAssertTrue(app.buttons["header.settings"].isHittable)
        let copy = app.buttons.matching(NSPredicate(format: "identifier == 'answer.copy' AND value == %@", "\(source.sourceBytes) bytes")).firstMatch
        XCTAssertTrue(copy.waitForExistence(timeout: 15))
        let copied = await copiedText(using: copy)
        let bytes = Data(try XCTUnwrap(copied).utf8)
        XCTAssertEqual(bytes.count, source.sourceBytes)
        XCTAssertEqual(SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined(), source.sourceSha256)
        XCTAssertTrue(wait { copy.label == "Copy" })
        capture("A ten-thousand-line native code fence stays bounded and keeps every original byte", screenshot: app.screenshot())
        app.terminate()
    }
}
