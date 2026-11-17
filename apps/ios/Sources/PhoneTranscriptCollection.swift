// Reuse visible SwiftUI cells while UIKit owns the transcript's scrolling and layout.
import SwiftUI
import UIKit

struct PhoneTranscriptCollection: UIViewControllerRepresentable {
    let host: PhoneHostModel
    let preferences: PhonePreferences
    let items: [PhoneTranscriptCellItem]
    let version: UUID
    let followRequest: UUID
    let session: String?
    let openQuestions: (PhoneApproval, PhoneQuestions) -> Void
    let readingChanged: (Bool) -> Void

    func makeUIViewController(context: Context) -> PhoneTranscriptController { PhoneTranscriptController() }
    func updateUIViewController(_ controller: PhoneTranscriptController, context: Context) { controller.update(self) }
}

@MainActor
final class PhoneTranscriptController: UIViewController, UICollectionViewDelegate, UICollectionViewDataSourcePrefetching {
    private let layout = PhoneTranscriptLayout()
    private lazy var collection = PhoneTranscriptNativeView(frame: .zero, collectionViewLayout: layout)
    private var source: UICollectionViewDiffableDataSource<Int, String>!
    private var configuration: PhoneTranscriptCollection?
    private var items: [String: PhoneTranscriptCellItem] = [:]
    private var order: [String] = []
    private var following = true
    private var userScrolling = false
    private var pending: PhoneTranscriptCollection?
    private var prefetched = Set<String>()
    private var followScheduled = false
    private var anchor: (id: String, y: CGFloat)?
    private var width: CGFloat = 0
    private var height: CGFloat = 0

    var isReadingGesture: Bool { userScrolling }

    override func loadView() {
        layout.owner = self
        collection.owner = self
        collection.backgroundColor = .clear
        collection.delegate = self
        collection.prefetchDataSource = self
        collection.keyboardDismissMode = .interactive
        collection.showsVerticalScrollIndicator = false
        collection.alwaysBounceVertical = true
        collection.contentInsetAdjustmentBehavior = .never
        collection.contentInset.bottom = 20
        collection.accessibilityIdentifier = "transcript"
        collection.selfSizingInvalidation = .enabledIncludingConstraints
        collection.register(PhoneTranscriptNativeCell.self, forCellWithReuseIdentifier: "row")
        source = UICollectionViewDiffableDataSource<Int, String>(collectionView: collection) { [weak self] view, path, id in
            let cell = view.dequeueReusableCell(withReuseIdentifier: "row", for: path)
            self?.configure(cell, id: id)
            return cell
        }
        view = collection
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if collection.bounds.width > 0, width != collection.bounds.width {
            width = collection.bounds.width
            layout.estimatedItemSize = CGSize(width: width, height: 26)
            layout.invalidateLayout()
            layoutChanged()
        }
        if height != collection.bounds.height {
            height = collection.bounds.height
            layoutChanged()
        }
    }

    func update(_ next: PhoneTranscriptCollection) {
        loadViewIfNeeded()
        let previous = configuration
        let changedSession = previous?.session != next.session
        let explicitFollow = previous?.followRequest != next.followRequest
        if userScrolling && !changedSession && !explicitFollow {
            pending = next
            next.host.prepareInline(Array(prefetched))
            return
        }
        if changedSession || explicitFollow { userScrolling = false; pending = nil }
        if changedSession || explicitFollow { following = true; anchor = nil }
        let saved = following ? nil : visibleAnchor()
        configuration = next
        items = Dictionary(uniqueKeysWithValues: next.items.map { ($0.id, $0) })
        let ids = next.items.map(\.id)
        let changed = order != ids
        if changed {
            let existing = Set(order)
            var snapshot = NSDiffableDataSourceSnapshot<Int, String>()
            snapshot.appendSections([0])
            snapshot.appendItems(ids)
            snapshot.reconfigureItems(collection.indexPathsForVisibleItems.compactMap {
                guard let id = source.itemIdentifier(for: $0), existing.contains(id), items[id] != nil else { return nil }
                return id
            })
            order = ids
            source.apply(snapshot, animatingDifferences: false) { [weak self] in
                guard let self else { return }
                guard !userScrolling else { return }
                if following { follow() }
                else { restore(anchor ?? saved) }
                publishReadingState()
            }
        } else {
            for path in collection.indexPathsForVisibleItems {
                if let id = source.itemIdentifier(for: path), let cell = collection.cellForItem(at: path) { configure(cell, id: id) }
            }
            if explicitFollow { follow() }
        }
    }

    private func configure(_ cell: UICollectionViewCell, id: String) {
        guard let configuration, let item = items[id] else { return }
        cell.backgroundColor = .clear
        cell.accessibilityIdentifier = "entry." + id
        cell.contentConfiguration = UIHostingConfiguration {
            PhoneTranscriptCellContent(host: configuration.host, item: item, loadHistory: { [weak self] in self?.loadHistory() }, openQuestions: configuration.openQuestions)
                .environment(configuration.preferences)
                .environment(\.preserveTranscriptPosition, { [weak self] id in self?.preserve(id) })
                .id(id)
        }.margins(.all, 0).minSize(width: 0, height: 0)
    }

    func shouldKeepPlace(above path: IndexPath) -> Bool {
        if following && !userScrolling { return true }
        if let anchor, let target = source.indexPath(for: anchor.id) { return path.item < target.item }
        return layout.layoutAttributesForItem(at: path).map { $0.frame.maxY <= collection.contentOffset.y + 0.5 } ?? false
    }

    func layoutChanged() {
        guard !followScheduled else { return }
        followScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            followScheduled = false
            if following && !userScrolling { follow() }
            else if !userScrolling { restore(anchor) }
            publishReadingState()
        }
    }

    private func visibleAnchor() -> (id: String, y: CGFloat)? {
        guard let path = collection.indexPathsForVisibleItems.sorted().first(where: { path in
            guard let id = source.itemIdentifier(for: path), let item = items[id] else { return false }
            if case .entry = item.content { return true }
            return false
        }),
              let id = source.itemIdentifier(for: path), let frame = layout.layoutAttributesForItem(at: path)?.frame else { return nil }
        return (id, contentTop(id, frame: frame) - collection.contentOffset.y)
    }

    private func preserve(_ id: String) {
        following = false
        if let path = source.indexPath(for: id), let frame = layout.layoutAttributesForItem(at: path)?.frame {
            anchor = (id, contentTop(id, frame: frame) - collection.contentOffset.y)
        } else { anchor = visibleAnchor() }
    }

    private func restore(_ saved: (id: String, y: CGFloat)?) {
        guard let saved, let path = source.indexPath(for: saved.id) else { return }
        collection.layoutIfNeeded()
        guard let frame = layout.layoutAttributesForItem(at: path)?.frame else { return }
        let target = contentTop(saved.id, frame: frame) - saved.y
        if abs(collection.contentOffset.y - target) > 0.5 {
            collection.setContentOffset(CGPoint(x: 0, y: target), animated: false)
        }
    }

    private func contentTop(_ id: String, frame: CGRect) -> CGFloat {
        guard let item = items[id], case .entry(let row) = item.content else { return frame.minY }
        return frame.minY + row.gap
    }

    private func follow() {
        guard !order.isEmpty else { return }
        collection.scrollToItem(at: IndexPath(item: order.count - 1, section: 0), at: .bottom, animated: false)
    }

    private var distance: CGFloat {
        max(0, collection.contentSize.height + collection.contentInset.bottom - collection.bounds.height - collection.contentOffset.y)
    }

    private func publishReadingState() { configuration?.readingChanged(!following && distance > 260) }

    private func loadHistory() {
        guard let host = configuration?.host, !host.history.loading, host.transcript?.hasOlder == true else { return }
        following = false
        anchor = visibleAnchor()
        host.history.load(host: host)
    }

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        userScrolling = true
        following = false
        anchor = nil
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        publishReadingState()
    }
    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { endedScrolling() }
    }
    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { endedScrolling() }

    private func endedScrolling() {
        guard userScrolling else { return }
        userScrolling = false
        following = distance <= 40
        if let next = pending { pending = nil; update(next) }
        if collection.contentOffset.y <= 20, configuration?.host.history.error == nil { loadHistory() }
        publishReadingState()
    }

    func collectionView(_ collectionView: UICollectionView, prefetchItemsAt indexPaths: [IndexPath]) {
        let entries = indexPaths.prefix(16).compactMap { path -> String? in
            guard let id = source.itemIdentifier(for: path), let item = items[id],
                  case .entry(let row) = item.content, !row.entry.isEmpty else { return nil }
            return row.entry
        }
        prefetched = Set(entries)
        guard let host = configuration?.host else { return }
        let missing = entries.filter { host.rows[$0] == nil }
        if !missing.isEmpty { host.loadRows(missing) }
        host.prepareInline(entries)
    }

    func collectionView(_ collectionView: UICollectionView, cancelPrefetchingForItemsAt indexPaths: [IndexPath]) {
        for path in indexPaths {
            guard let id = source.itemIdentifier(for: path), let item = items[id], case .entry(let row) = item.content else { continue }
            prefetched.remove(row.entry)
        }
    }
}
