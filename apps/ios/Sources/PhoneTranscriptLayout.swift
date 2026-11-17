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

    override func layoutSubviews() {
        super.layoutSubviews()
        if contentSize != measuredSize {
            measuredSize = contentSize
            owner?.layoutChanged()
        }
    }
}
