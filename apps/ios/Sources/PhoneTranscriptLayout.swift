// Let UIKit apply size changes above the reader during its own layout transaction.
import UIKit

@MainActor
final class PhoneTranscriptLayout: UICollectionViewFlowLayout {
    weak var owner: PhoneTranscriptController?

    override init() {
        super.init()
        minimumLineSpacing = 0
        minimumInteritemSpacing = 0
        sectionInset = .zero
    }

    required init?(coder: NSCoder) { nil }

    override func invalidationContext(
        forPreferredLayoutAttributes preferred: UICollectionViewLayoutAttributes,
        withOriginalAttributes original: UICollectionViewLayoutAttributes
    ) -> UICollectionViewLayoutInvalidationContext {
        let context = super.invalidationContext(forPreferredLayoutAttributes: preferred, withOriginalAttributes: original)
        let delta = preferred.size.height - original.size.height
        if abs(delta) > 0.5, owner?.shouldKeepPlace(above: original.indexPath) == true {
            context.contentOffsetAdjustment.y += delta
        }
        owner?.layoutChanged()
        return context
    }
}

@MainActor
final class PhoneTranscriptNativeCell: UICollectionViewCell {
    override func preferredLayoutAttributesFitting(_ attributes: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        let result = attributes.copy() as! UICollectionViewLayoutAttributes
        let size = contentView.systemLayoutSizeFitting(
            CGSize(width: attributes.size.width, height: UIView.layoutFittingCompressedSize.height),
            withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
        result.size = CGSize(width: attributes.size.width, height: max(1, ceil(size.height)))
        return result
    }
}

@MainActor
final class PhoneTranscriptNativeView: UICollectionView {
    weak var owner: PhoneTranscriptController?
    private var measuredSize = CGSize.zero
    private var restoringReader = false

    override func layoutSubviews() {
        let keepPlace = owner?.isReadingGesture == true && !restoringReader
            && contentOffset.y >= 0
            && contentOffset.y <= max(0, contentSize.height + contentInset.bottom - bounds.height)
        let before = keepPlace ? visibleCells.sorted { $0.frame.minY < $1.frame.minY }.compactMap { cell -> (String, CGFloat, CGFloat, CGFloat)? in
            guard let id = cell.accessibilityIdentifier, id.hasPrefix("entry."),
                  cell.frame.maxY > contentOffset.y, cell.frame.minY < contentOffset.y + bounds.height else { return nil }
            return (id, cell.frame.minY - contentOffset.y, cell.frame.minY, cell.frame.height)
        } : []
        super.layoutSubviews()
        // A pan already moved the viewport before this layout pass. Preserve that
        // position only when measuring cells moves their actual content geometry.
        for (id, y, top, height) in before {
            guard let cell = visibleCells.first(where: { $0.accessibilityIdentifier == id }) else { continue }
            if abs(cell.frame.minY - top) > 0.5 || abs(cell.frame.height - height) > 0.5 {
                let delta = cell.frame.minY - contentOffset.y - y
                if abs(delta) > 0.5 {
                    restoringReader = true
                    setContentOffset(CGPoint(x: contentOffset.x, y: contentOffset.y + delta), animated: false)
                    restoringReader = false
                }
            }
            break
        }
        if contentSize != measuredSize {
            measuredSize = contentSize
            owner?.layoutChanged()
        }
    }
}
