// Compare Carlos with UIKit's unmodified scrolling in the same viewport; never compiled into the shipping app.
#if CARLOS_FIXTURE
import SwiftUI
import UIKit

struct PhoneNativeScrollReference: UIViewControllerRepresentable {
    let contentHeight: CGFloat

    func makeUIViewController(context: Context) -> NativeTextPanel { NativeTextPanel(contentHeight: contentHeight) }
    func updateUIViewController(_ controller: NativeTextPanel, context: Context) {}
}

@MainActor
final class NativeTextPanel: UIViewController, UIScrollViewDelegate {
    private let scroll = UIScrollView()
    private let text = UILabel()
    private let contentHeight: CGFloat
    private var positioned = false

    init(contentHeight: CGFloat) { self.contentHeight = contentHeight; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { nil }

    override func loadView() {
        scroll.delegate = self
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.contentInset.bottom = 20
        scroll.alwaysBounceVertical = true
        scroll.accessibilityIdentifier = "transcript.native-reference"
        text.numberOfLines = 0
        text.font = UIFont(name: "InterVariable", size: 15)
        text.textColor = .white
        text.text = String(repeating: "Ordinary native iOS text. Scroll through these lines.\n", count: 1000)
        scroll.addSubview(text)
        view = scroll
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if scroll.contentSize.width != scroll.bounds.width || scroll.contentSize.height != contentHeight {
            text.frame = CGRect(x: 16, y: 0, width: max(0, scroll.bounds.width - 32), height: contentHeight)
            scroll.contentSize = CGSize(width: scroll.bounds.width, height: contentHeight)
        }
        if !positioned, scroll.bounds.height > 0 {
            positioned = true
            scroll.contentOffset.y = max(0, contentHeight - scroll.bounds.height)
        }
        publish()
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) { publish() }

    private func publish() {
        scroll.accessibilityValue = "\(scroll.contentOffset.y)/\(scroll.bounds.height)/\(scroll.contentSize.height)/\(-scroll.contentOffset.y)"
    }
}
#endif
