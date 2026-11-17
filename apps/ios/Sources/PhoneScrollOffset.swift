// Set a measured point once without retaining a SwiftUI programmatic scroll target during a gesture.
import SwiftUI
import UIKit

@MainActor
final class PhoneScrollOffset {
    weak var view: UIScrollView?
    var rows: [String: WeakMarker] = [:]
    var y: CGFloat { view?.contentOffset.y ?? 0 }

    var frames: [String: CGRect] {
        guard let view else { return [:] }
        rows = rows.filter { $0.value.view != nil }
        return rows.compactMapValues { reference in
            guard let marker = reference.view, marker.window != nil else { return nil }
            let frame = marker.convert(marker.bounds, to: view)
            return CGRect(x: frame.minX - view.bounds.minX, y: frame.minY - view.bounds.minY + marker.gap,
                          width: frame.width, height: max(0, frame.height - marker.gap))
        }
    }

    func scroll(to y: CGFloat) {
        guard let view, y.isFinite else { return }
        let lower = -view.adjustedContentInset.top
        let upper = max(lower, view.contentSize.height - view.bounds.height + view.adjustedContentInset.bottom)
        view.setContentOffset(CGPoint(x: view.contentOffset.x, y: min(upper, max(lower, y))), animated: false)
    }

    final class WeakMarker {
        weak var view: PhoneScrollAttachment.Marker?
        init(_ view: PhoneScrollAttachment.Marker) { self.view = view }
    }
}

// The marker sits inside the transcript content, so its nearest scroll ancestor is the transcript itself.
struct PhoneScrollAttachment: UIViewRepresentable {
    let offset: PhoneScrollOffset
    var row: String? = nil
    var gap: CGFloat = 0

    func makeUIView(context: Context) -> Marker { Marker(offset: offset, row: row, gap: gap) }
    func updateUIView(_ view: Marker, context: Context) { view.gap = gap }

    static func dismantleUIView(_ view: Marker, coordinator: ()) {
        if let row = view.row, view.offset.rows[row]?.view === view {
            view.offset.rows.removeValue(forKey: row)
        }
    }

    final class Marker: UIView {
        let offset: PhoneScrollOffset
        let row: String?
        var gap: CGFloat

        init(offset: PhoneScrollOffset, row: String?, gap: CGFloat) {
            self.offset = offset
            self.row = row
            self.gap = gap
            super.init(frame: .zero)
            isUserInteractionEnabled = false
            isAccessibilityElement = false
            if let row { offset.rows[row] = PhoneScrollOffset.WeakMarker(self) }
        }

        required init?(coder: NSCoder) { nil }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            var ancestor = superview
            while let current = ancestor {
                if let scroll = current as? UIScrollView { offset.view = scroll; return }
                ancestor = current.superview
            }
        }
    }
}
