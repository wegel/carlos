// Keep SwiftUI keyboard controls inside UIKit's own full-width accessory view.
import SwiftUI
import UIKit

final class PhoneInputAccessory<Content: View>: UIView {
    private let hosted: any UIView & UIContentView

    init(content: Content) {
        hosted = UIHostingConfiguration { content }.margins(.all, 0).minSize(width: 0, height: 44).makeContentView()
        super.init(frame: CGRect(x: 0, y: 0, width: 0, height: 44))
        autoresizingMask = .flexibleWidth
        backgroundColor = UIColor(PhoneTheme.mantle)
        hosted.backgroundColor = .clear
        hosted.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        addSubview(hosted)
    }

    required init?(coder: NSCoder) { fatalError("Create the keyboard toolbar with its content") }

    override var intrinsicContentSize: CGSize { CGSize(width: UIView.noIntrinsicMetric, height: 44) }

    override func layoutSubviews() {
        super.layoutSubviews()
        hosted.frame = bounds
    }

    func update(_ content: Content) {
        hosted.configuration = UIHostingConfiguration { content }.margins(.all, 0).minSize(width: 0, height: 44)
    }
}
