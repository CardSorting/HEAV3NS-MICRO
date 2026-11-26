// [LAYER: UI]
import AppKit

struct GitHubRemoteStatus {
    struct RepoOption {
        let nameWithOwner: String
        let url: String
        let isPrivate: Bool
    }

    let ghInstalled: Bool
    let ghAuthenticated: Bool
    let ghUser: String
    let hasRemote: Bool
    let webUrl: String
    let remoteName: String
    let suggestedName: String
    let repos: [RepoOption]

    static let empty = GitHubRemoteStatus(
        ghInstalled: false, ghAuthenticated: false, ghUser: "",
        hasRemote: false, webUrl: "", remoteName: "", suggestedName: "", repos: []
    )

    static func parse(_ data: [String: Any]) -> GitHubRemoteStatus {
        let rawRepos = data["repos"] as? [[String: Any]] ?? []
        let repos = rawRepos.compactMap { item -> RepoOption? in
            guard let name = item["nameWithOwner"] as? String, !name.isEmpty,
                  let url = item["url"] as? String, !url.isEmpty else { return nil }
            return RepoOption(nameWithOwner: name, url: url, isPrivate: item["isPrivate"] as? Bool ?? true)
        }
        return GitHubRemoteStatus(
            ghInstalled: data["ghInstalled"] as? Bool ?? false,
            ghAuthenticated: data["ghAuthenticated"] as? Bool ?? false,
            ghUser: data["ghUser"] as? String ?? "",
            hasRemote: data["hasRemote"] as? Bool ?? false,
            webUrl: data["webUrl"] as? String ?? "",
            remoteName: data["remoteName"] as? String ?? "",
            suggestedName: data["suggestedName"] as? String ?? "",
            repos: repos
        )
    }
}

/// One-click, on-the-fly GitHub repository creator for SourceControlWindow.
/// Defaults to a single obvious "Create & Publish to GitHub" action with a pre-filled repo name
/// and Private/Public selector; advanced link/existing-repo options stay tucked inside "⋯".
final class SourceControlRemoteBar: NSView {
    var onCreateAndPublish: ((String, String) -> Void)?
    var onConnectRepo: ((String) -> Void)?
    var onSignIn: (() -> Void)?
    var onCheckStatus: (() -> Void)?

    private let stack = NSStackView()
    private let statusLabel = NSTextField(labelWithString: "Back up & publish your project to GitHub")
    private let repoNameField = NSTextField(string: "")
    private let visibilityPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let morePopup = NSPopUpButton(frame: .zero, pullsDown: true)
    private let customURLField = NSTextField(string: "")
    private var createPublishBtn: JourneyButton!
    private var signInBtn: JourneyButton!
    private var checkAgainBtn: JourneyButton!
    private var connectLinkBtn: JourneyButton!
    private var cancelLinkBtn: JourneyButton!

    private var repoOptions = [GitHubRemoteStatus.RepoOption]()
    private var showingManualLink = false
    private var awaitingSignIn = false
    private var isPublishing = false
    private var currentStatus = GitHubRemoteStatus.empty

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.06).cgColor
        buildUI()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    private func buildUI() {
        statusLabel.font = .systemFont(ofSize: 11.5, weight: .medium)
        statusLabel.textColor = .secondaryLabelColor
        statusLabel.lineBreakMode = .byTruncatingTail
        statusLabel.setContentHuggingPriority(.init(1), for: .horizontal)

        repoNameField.placeholderString = "New repository name"
        repoNameField.font = .systemFont(ofSize: 11.5)
        repoNameField.controlSize = .small
        repoNameField.toolTip = "Name for your new GitHub repository"
        repoNameField.widthAnchor.constraint(greaterThanOrEqualToConstant: 140).isActive = true
        repoNameField.widthAnchor.constraint(lessThanOrEqualToConstant: 210).isActive = true

        visibilityPopup.controlSize = .small
        visibilityPopup.addItems(withTitles: ["Private", "Public"])
        visibilityPopup.selectItem(at: 0)
        visibilityPopup.toolTip = "Choose whether the new GitHub repository is Private or Public"

        createPublishBtn = JourneyButton("Create & Publish to GitHub", symbol: "icloud.and.arrow.up") { [weak self] in
            guard let self, !self.isPublishing else { return }
            let name = self.repoNameField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            let visibility = self.visibilityPopup.indexOfSelectedItem == 1 ? "public" : "private"
            self.setPublishing(true)
            self.onCreateAndPublish?(name, visibility)
        }
        createPublishBtn.controlSize = .small
        createPublishBtn.bezelStyle = .rounded
        createPublishBtn.bezelColor = .controlAccentColor

        morePopup.controlSize = .small
        morePopup.bezelStyle = .rounded
        morePopup.target = self
        morePopup.action = #selector(selectMoreOption)
        morePopup.toolTip = "More GitHub options"

        signInBtn = JourneyButton("Sign in to GitHub…", symbol: "person.crop.circle.badge.checkmark") { [weak self] in
            guard let self else { return }
            self.awaitingSignIn = true
            self.applyState()
            self.onSignIn?()
        }
        signInBtn.controlSize = .small
        signInBtn.bezelStyle = .rounded
        signInBtn.bezelColor = .controlAccentColor

        checkAgainBtn = JourneyButton("Check Sign-In", symbol: "arrow.clockwise") { [weak self] in
            self?.onCheckStatus?()
        }
        checkAgainBtn.controlSize = .small
        checkAgainBtn.bezelStyle = .rounded

        customURLField.placeholderString = "Paste GitHub link or owner/project…"
        customURLField.font = .systemFont(ofSize: 11.5)
        customURLField.controlSize = .small
        customURLField.setContentHuggingPriority(.init(1), for: .horizontal)

        connectLinkBtn = JourneyButton("Connect", symbol: "link") { [weak self] in
            guard let self else { return }
            let raw = self.customURLField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !raw.isEmpty else { return }
            self.onConnectRepo?(raw)
            self.customURLField.stringValue = ""
            self.showingManualLink = false
            self.applyState()
        }
        connectLinkBtn.controlSize = .small
        connectLinkBtn.bezelStyle = .rounded
        connectLinkBtn.bezelColor = .controlAccentColor

        cancelLinkBtn = JourneyButton("Cancel", symbol: "xmark") { [weak self] in
            guard let self else { return }
            self.showingManualLink = false
            self.applyState()
        }
        cancelLinkBtn.controlSize = .small
        cancelLinkBtn.bezelStyle = .rounded

        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 6, left: 14, bottom: 6, right: 14)
        stack.translatesAutoresizingMaskIntoConstraints = false
        [statusLabel, repoNameField, visibilityPopup, createPublishBtn, signInBtn, checkAgainBtn, customURLField, connectLinkBtn, cancelLinkBtn, morePopup]
            .forEach(stack.addArrangedSubview)

        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor)
        ])
        rebuildMoreMenu()
        applyState()
    }

    func setPublishing(_ busy: Bool) {
        isPublishing = busy
        createPublishBtn.isEnabled = !busy
        createPublishBtn.title = busy ? "Creating & Publishing…" : "Create & Publish to GitHub"
        repoNameField.isEnabled = !busy
        visibilityPopup.isEnabled = !busy
        morePopup.isEnabled = !busy
    }

    func update(status: GitHubRemoteStatus) {
        currentStatus = status
        repoOptions = status.repos
        if repoNameField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !status.suggestedName.isEmpty {
            repoNameField.stringValue = status.suggestedName
        }
        if status.ghAuthenticated {
            awaitingSignIn = false
        }
        setPublishing(false)
        rebuildMoreMenu()
        applyState()
    }

    private func rebuildMoreMenu() {
        morePopup.removeAllItems()
        morePopup.addItem(withTitle: "⋯")
        morePopup.addItem(withTitle: "Paste Repository Link…")
        if !repoOptions.isEmpty {
            morePopup.menu?.addItem(.separator())
            let header = NSMenuItem(title: "Or Connect Existing Repo", action: nil, keyEquivalent: "")
            header.isEnabled = false
            morePopup.menu?.addItem(header)
            for repo in repoOptions {
                let icon = repo.isPrivate ? "🔒 " : "🌐 "
                let item = NSMenuItem(title: "\(icon)\(repo.nameWithOwner)", action: nil, keyEquivalent: "")
                item.representedObject = repo.url
                morePopup.menu?.addItem(item)
            }
        }
    }

    @objc private func selectMoreOption() {
        guard let selected = morePopup.selectedItem else { return }
        if selected.title == "Paste Repository Link…" {
            showingManualLink = true
            applyState()
            window?.makeFirstResponder(customURLField)
        } else if let url = selected.representedObject as? String, !url.isEmpty {
            onConnectRepo?(url)
        }
        morePopup.selectItem(at: 0)
    }

    private func applyState() {
        if showingManualLink {
            statusLabel.isHidden = true
            repoNameField.isHidden = true
            visibilityPopup.isHidden = true
            createPublishBtn.isHidden = true
            signInBtn.isHidden = true
            checkAgainBtn.isHidden = true
            morePopup.isHidden = true
            customURLField.isHidden = false
            connectLinkBtn.isHidden = false
            cancelLinkBtn.isHidden = false
            return
        }

        customURLField.isHidden = true
        connectLinkBtn.isHidden = true
        cancelLinkBtn.isHidden = true

        if currentStatus.ghAuthenticated {
            statusLabel.isHidden = false
            statusLabel.stringValue = "GitHub • @\(currentStatus.ghUser)"
            repoNameField.isHidden = false
            visibilityPopup.isHidden = false
            createPublishBtn.isHidden = false
            morePopup.isHidden = false
            signInBtn.isHidden = true
            checkAgainBtn.isHidden = true
        } else {
            statusLabel.isHidden = false
            statusLabel.stringValue = awaitingSignIn
                ? "Finish signing in on GitHub, then click Check Sign-In"
                : "Back up & publish your project to GitHub"
            repoNameField.isHidden = true
            visibilityPopup.isHidden = true
            createPublishBtn.isHidden = true
            morePopup.isHidden = false
            signInBtn.isHidden = false
            checkAgainBtn.isHidden = !awaitingSignIn
        }
    }

    func dispose() {
        onCreateAndPublish = nil
        onConnectRepo = nil
        onSignIn = nil
        onCheckStatus = nil
    }
}
