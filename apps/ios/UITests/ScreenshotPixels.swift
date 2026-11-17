// Read actual simulator pixels without changing the screenshot or its color channels.
import UIKit
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func rgbaPixels(_ screenshot: XCUIScreenshot, frame: CGRect, screen: CGRect) throws -> [UInt8] {
        let image = try XCTUnwrap(screenshot.image.cgImage)
        let x = CGFloat(image.width) / screen.width, y = CGFloat(image.height) / screen.height
        let region = CGRect(x: frame.minX * x, y: frame.minY * y, width: frame.width * x, height: frame.height * y).integral
        let crop = try XCTUnwrap(image.cropping(to: region))
        var bytes = [UInt8](repeating: 0, count: crop.width * crop.height * 4)
        try bytes.withUnsafeMutableBytes { buffer in
            let context = try XCTUnwrap(CGContext(data: buffer.baseAddress, width: crop.width, height: crop.height,
                bitsPerComponent: 8, bytesPerRow: crop.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue))
            context.draw(crop, in: CGRect(x: 0, y: 0, width: crop.width, height: crop.height))
        }
        return bytes
    }
}
