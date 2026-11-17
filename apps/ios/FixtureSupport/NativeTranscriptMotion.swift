// Count movement of the same visible text, independent of revised offscreen height estimates.
#if CARLOS_FIXTURE
import UIKit

@MainActor
final class NativeTranscriptMotion {
    private var positions: [String: CGFloat] = [:]
    private(set) var distance: CGFloat = 0

    func record(_ view: UICollectionView) {
        let cells = view.visibleCells.sorted { $0.frame.minY < $1.frame.minY }
        let next = cells.compactMap { cell -> (String, CGFloat)? in
            guard let id = cell.accessibilityIdentifier, id.hasPrefix("entry.") else { return nil }
            return (id, cell.frame.minY - view.contentOffset.y)
        }
        if let match = next.first(where: { positions[$0.0] != nil }), let old = positions[match.0] {
            distance += match.1 - old
        }
        positions = Dictionary(uniqueKeysWithValues: next)
    }
}
#endif
