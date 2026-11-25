// [LAYER: UI]
import AppKit

/// Manage every signed-in ChatGPT account in one place: see plan and usage-limit status, switch, rename, remove, add,
/// and choose whether HEAV3NS switches automatically when an account reaches its limit. Switching takes effect for
/// running workspaces on their next request, so there is no need to close anything first.
final class ChatGPTAccountsWindow: NSWindowController, NSWindowDelegate {
    static let shared = ChatGPTAccountsWindow()
    /// Posted after the account list changes so other surfaces (the model picker) can refresh what they show.
    static let didChange = Notification.Name("ChatGPTAccountsDidChange")

    private let client: AccountClient
    private let root = JourneyStyle.stack([], spacing: 12)
    private var list: ChatGPTAccountList?
    private var message = ""
    private var loaded = false
    private var waiting = false
    private var cancelling = false
    private var busy = false
    private var pendingRequests = 0
    private var requestToken: UInt64 = 0
    private var browserURL: URL?
    private var clock: Timer?
    private var accountsObserver: NSObjectProtocol?
    var now: () -> Date = Date.init

    init(client: AccountClient = AccountClient()) {
        self.client = client
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 620, height: 420), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.title = "ChatGPT Accounts"; window.minSize = NSSize(width: 520, height: 300); window.isReleasedWhenClosed = false
        super.init(window: window); window.delegate = self; window.center()
        let scroll = NSScrollView(); scroll.hasVerticalScroller = true; scroll.drawsBackground = false
        let document = FlippedView(); document.translatesAutoresizingMaskIntoConstraints = false
        scroll.documentView = document
        document.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: document.leadingAnchor, constant: 28), root.trailingAnchor.constraint(equalTo: document.trailingAnchor, constant: -28),
            root.topAnchor.constraint(equalTo: document.topAnchor, constant: 24), root.bottomAnchor.constraint(equalTo: document.bottomAnchor, constant: -24),
            document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor)
        ])
        window.contentView = scroll
        client.onBrowser = { [weak self] url in
            guard let self else { return }
            self.browserURL = url
            if !NSWorkspace.shared.open(url) { self.message = "The browser could not open. Use Open Browser to try again." }
            self.render()
        }
        accountsObserver = NotificationCenter.default.addObserver(forName: Self.didChange, object: nil, queue: .main) { [weak self] note in
            guard let self, note.object as AnyObject? !== self, self.window?.isVisible == true, !self.waiting else { return }
            self.refresh()
        }
        render()
    }
    deinit {
        clock?.invalidate()
        if let accountsObserver { NotificationCenter.default.removeObserver(accountsObserver) }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    func present() {
        showWindow(nil); window?.makeKeyAndOrderFront(nil)
        refresh()
        // Limit countdowns and "back in" times stay honest while the window is open.
        clock?.invalidate()
        clock = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in self?.render() }
    }
    func windowWillClose(_ notification: Notification) {
        clock?.invalidate(); clock = nil
        // Closing the window also ends any browser sign-in still waiting (EOF cancels it) and frees the helper process.
        waiting = false; cancelling = false; browserURL = nil; loaded = false; pendingRequests = 0; busy = false; requestToken &+= 1
        client.stop()
    }
    override func cancelOperation(_ sender: Any?) {
        if !waiting { window?.performClose(sender) }
    }

    // MARK: Requests

    private func refresh() { perform("accounts") }

    private func perform(_ method: String, params: [String: Any] = [:], success: String = "") {
        guard !busy || method == "accounts" else { return }
        requestToken &+= 1
        let token = requestToken
        pendingRequests += 1
        busy = true; render()
        client.request(method, params: params) { [weak self] result in
            guard let self else { return }
            self.pendingRequests = max(0, self.pendingRequests - 1)
            self.busy = self.pendingRequests > 0
            self.loaded = true
            guard method != "accounts" || token == self.requestToken else { self.render(); return }
            switch result {
            case .success(let data):
                if let updated = ChatGPTAccountList(data) {
                    self.list = updated; self.message = success
                    NotificationCenter.default.post(name: Self.didChange, object: self)
                } else { self.message = "The account service returned an unexpected response. Try again." }
            case .failure(let error): self.message = error.localizedDescription
            }
            self.render()
        }
    }

    private func addAccount() {
        guard !waiting else { return }
        waiting = true; cancelling = false; browserURL = nil; message = ""; render()
        client.request("signIn") { [weak self] result in
            guard let self else { return }
            self.waiting = false; self.cancelling = false; self.browserURL = nil; self.loaded = true
            switch result {
            case .success(let data):
                if let updated = ChatGPTAccountList(data) { self.list = updated }
                let added = (data["account"] as? [String: Any])?["label"] as? String
                let created = data["created"] as? Bool ?? true
                self.message = added.map { created ? "Added \"\($0)\" and made it active." : "Refreshed \"\($0)\"." } ?? "Signed in."
                if data["accessChecked"] as? Bool == false { self.message += " Model access could not be confirmed; check the account or try again." }
                NotificationCenter.default.post(name: Self.didChange, object: self)
            case .failure(let error): self.message = error.localizedDescription
            }
            self.render()
        }
    }

    private func cancelSignIn() {
        guard !cancelling else { return }
        cancelling = true; message = "Cancelling sign-in…"; render()
        client.request("cancel") { _ in }
    }

    // MARK: Actions

    private func rename(_ account: ChatGPTAccount) {
        let field = NSTextField(string: account.label); field.frame = NSRect(x: 0, y: 0, width: 260, height: 24)
        field.setAccessibilityLabel("Account name"); field.placeholderString = "Work, Personal…"
        let alert = NSAlert(); alert.messageText = "Rename account"; alert.informativeText = "Choose a name that helps you recognise this account."
        alert.accessoryView = field; alert.addButton(withTitle: "Rename"); alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        perform("rename", params: ["account": account.id, "label": field.stringValue])
    }

    private func remove(_ account: ChatGPTAccount) {
        let alert = NSAlert(); alert.alertStyle = .warning
        alert.messageText = "Remove \"\(account.label)\"?"
        alert.informativeText = "This forgets the sign-in on this Mac. It does not sign you out of chatgpt.com, and you can add the account again at any time."
        let remove = alert.addButton(withTitle: "Remove"); remove.hasDestructiveAction = true
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        perform("remove", params: ["account": account.id], success: "Removed \"\(account.label)\".")
    }

    // MARK: Rendering

    private func render() {
        for view in root.arrangedSubviews { root.removeArrangedSubview(view); view.removeFromSuperview() }
        let current = now()
        var views: [NSView] = [JourneyStyle.label("ChatGPT accounts", size: 26, weight: .semibold)]
        let subtitle = waiting ? "Continue in your browser. You can cancel and try again at any time."
            : "Add the ChatGPT accounts you use. When one reaches its usage limit, HEAV3NS can continue on another without interrupting your work."
        views.append(JourneyStyle.label(subtitle, size: 13, secondary: true))

        if let list {
            if list.accounts.isEmpty {
                views.append(JourneyStyle.label("No account is signed in yet.", size: 14))
            } else {
                views.append(JourneyStyle.rule())
                for (index, account) in list.accounts.enumerated() {
                    if index > 0 { views.append(JourneyStyle.rule()) }
                    views.append(row(account, now: current))
                }
                views.append(JourneyStyle.rule())
            }
        } else if !loaded {
            let progress = NSProgressIndicator(); progress.style = .spinning; progress.controlSize = .small; progress.startAnimation(nil)
            views.append(progress)
        }

        if waiting {
            let progress = NSProgressIndicator(); progress.style = .spinning; progress.controlSize = .small; progress.startAnimation(nil)
            var waitingViews: [NSView] = [progress]
            if let browserURL {
                waitingViews.append(JourneyButton("Open Browser", symbol: "arrow.up.right.square") { NSWorkspace.shared.open(browserURL) })
                waitingViews.append(JourneyButton("Copy Link", symbol: "doc.on.doc") { [weak self] in
                    NSPasteboard.general.clearContents(); NSPasteboard.general.setString(browserURL.absoluteString, forType: .string)
                    self?.message = "Sign-in link copied. Paste it into any browser or profile to continue."; self?.render()
                })
            }
            let cancel = JourneyButton(cancelling ? "Cancelling…" : "Cancel sign-in") { [weak self] in self?.cancelSignIn() }
            cancel.isEnabled = !cancelling
            waitingViews.append(cancel)
            views.append(JourneyStyle.stack(waitingViews, vertical: false, spacing: 10))
        } else {
            let add = JourneyButton(list?.accounts.isEmpty == false ? "Add another account…" : "Sign in with ChatGPT", symbol: "plus") { [weak self] in self?.addAccount() }
            add.bezelColor = .controlAccentColor; add.isEnabled = !busy
            let refresh = JourneyButton("Refresh", symbol: "arrow.clockwise") { [weak self] in self?.refresh() }
            refresh.isEnabled = !busy
            views.append(JourneyStyle.stack([add, refresh, NSView()], vertical: false, spacing: 8))
        }

        if let list, !list.accounts.isEmpty {
            let auto = NSButton(checkboxWithTitle: "Switch automatically when an account reaches its usage limit", target: self, action: #selector(toggleAuto(_:)))
            auto.state = list.autoSwitch ? .on : .off; auto.isEnabled = !busy
            auto.setAccessibilityLabel("Switch automatically when an account reaches its usage limit")
            views.append(auto)
            views.append(JourneyStyle.label(list.autoSwitch ? "Work continues on the next ready account, and the switch is shown in the conversation."
                                                            : "Off: you'll be told when an account is at its limit and can switch yourself.", size: 12, secondary: true))
            views.append(JourneyStyle.label(list.summary(now: current), size: 12, secondary: true))
        }
        if !message.isEmpty {
            let box = NSBox(); box.boxType = .custom; box.cornerRadius = 10
            box.fillColor = .controlBackgroundColor; box.borderColor = .separatorColor; box.borderWidth = 1
            box.contentViewMargins = NSSize(width: 12, height: 10)
            let notice = JourneyStyle.label(message, size: 13); notice.isSelectable = true
            box.addSubview(notice); JourneyStyle.pin(notice, to: box, inset: 10)
            views.append(box)
        }
        for view in views { root.addArrangedSubview(view); if view is NSBox || view is NSStackView { view.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true } }
        window?.contentView?.needsLayout = true
    }

    private func row(_ account: ChatGPTAccount, now: Date) -> NSView {
        let marker = NSImageView(image: NSImage(systemSymbolName: account.active ? "checkmark.circle.fill" : "circle", accessibilityDescription: account.active ? "Active account" : nil) ?? NSImage())
        marker.contentTintColor = account.active ? .controlAccentColor : .tertiaryLabelColor
        marker.widthAnchor.constraint(equalToConstant: 20).isActive = true
        let title = JourneyStyle.label(account.label + (account.planName.map { "  ·  \($0)" } ?? ""), size: 14, weight: .semibold)
        title.maximumNumberOfLines = 1; title.lineBreakMode = .byTruncatingTail
        let status = JourneyStyle.label((account.active ? "Active · " : "") + account.statusText(now: now), size: 12, secondary: true)
        status.textColor = account.isLimited(now: now) ? .systemOrange : .secondaryLabelColor
        let text = JourneyStyle.stack([title, status], spacing: 2)
        text.setContentHuggingPriority(.init(1), for: .horizontal)
        var controls: [NSView] = [marker, text, NSView()]
        if !account.active {
            let use = JourneyButton("Use this account") { [weak self] in self?.perform("switch", params: ["account": account.id], success: "Switched to \"\(account.label)\". Running workspaces use it from their next request.") }
            use.setAccessibilityLabel("Switch to \(account.label)"); use.isEnabled = !busy; controls.append(use)
        }
        let more = JourneyButton("Rename…") { [weak self] in self?.rename(account) }; more.setAccessibilityLabel("Rename \(account.label)"); more.isEnabled = !busy
        let remove = JourneyButton("Remove…") { [weak self] in self?.remove(account) }; remove.setAccessibilityLabel("Remove \(account.label)"); remove.isEnabled = !busy
        controls += [more, remove]
        if account.isLimited(now: now) {
            let clear = JourneyButton("Clear limit") { [weak self] in self?.perform("clearLimit", params: ["account": account.id], success: "Cleared the recorded limit for \"\(account.label)\".") }
            clear.toolTip = "Forget the recorded usage limit, for example after upgrading the plan."; clear.setAccessibilityLabel("Clear recorded limit for \(account.label)"); clear.isEnabled = !busy
            controls.insert(clear, at: controls.count - 2)
        }
        let line = JourneyStyle.stack(controls, vertical: false, spacing: 8)
        line.setAccessibilityLabel("\(account.label), \(account.statusText(now: now))\(account.active ? ", active" : "")")
        return line
    }

    @objc private func toggleAuto(_ sender: NSButton) {
        perform("setAuto", params: ["enabled": sender.state == .on], success: sender.state == .on ? "Automatic switching is on." : "Automatic switching is off.")
    }
    func dispose() {
        clock?.invalidate(); clock = nil
        if let accountsObserver { NotificationCenter.default.removeObserver(accountsObserver); self.accountsObserver = nil }
        waiting = false; cancelling = false; browserURL = nil; loaded = false; pendingRequests = 0; busy = false; requestToken &+= 1
        client.stop()
        window?.orderOut(nil)
    }
}

private final class FlippedView: NSView { override var isFlipped: Bool { true } }
