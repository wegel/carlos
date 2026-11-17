// Use UIKit’s native list layout for self-sizing SwiftUI transcript cells.
import UIKit

@MainActor
final class PhoneTranscriptLayout: UICollectionViewCompositionalLayout {
    init() {
        super.init { _, environment in
            var configuration = UICollectionLayoutListConfiguration(appearance: .plain)
            configuration.showsSeparators = false
            configuration.backgroundColor = .clear
            let section = NSCollectionLayoutSection.list(using: configuration, layoutEnvironment: environment)
            section.contentInsets = .zero
            return section
        }
    }

    required init?(coder: NSCoder) { nil }
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
    #if CARLOS_FIXTURE
    let motion = NativeTranscriptMotion()
    #endif

    override func layoutSubviews() {
        #if CARLOS_FIXTURE
        motion.record(self)
        #endif
        super.layoutSubviews()
        #if CARLOS_FIXTURE
        motion.record(self)
        #endif
        if contentSize != measuredSize {
            measuredSize = contentSize
            owner?.layoutChanged()
        }
    }
}
