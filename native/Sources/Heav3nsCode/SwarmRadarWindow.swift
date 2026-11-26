// [LAYER: INFRASTRUCTURE]
import AppKit

final class SwarmRadarWindow: NSWindowController, NSWindowDelegate {
    static let shared = SwarmRadarWindow()

    private let statusPill = JourneyStyle.label("STANDBY", size: 11, weight: .bold)
    private let titleLabel = JourneyStyle.label("Sovereign Swarm Radar", size: 18, weight: .bold)
    private let nodeIdLabel = JourneyStyle.label("NODE_LOCAL", size: 11, weight: .bold)
    private let statsSummaryLabel = JourneyStyle.label("WAVE 1 · SCORE 0 · ⚡ 75 · HULL 100%", size: 11.5, weight: .semibold)

    private let deployInterceptorBtn = JourneyButton("[1] Fighter (25⚡)", symbol: "paperplane.fill") {}
    private let deployTurretBtn = JourneyButton("[2] Turret (40⚡)", symbol: "scope") {}
    private let deployHeavyBtn = JourneyButton("[3] Gundam (50⚡)", symbol: "shield.lefthalf.filled") {}
    private let overdriveBtn = JourneyButton("[4] Overdrive (60⚡)", symbol: "bolt.shield.fill") {}
    private let orbitalStrikeBtn = JourneyButton("[5] Orbital Strike (75⚡)", symbol: "sparkles") {}
    private let resetMissionBtn = JourneyButton("[R] Reset", symbol: "arrow.counterclockwise") {}

    private let copyButton = JourneyButton("Copy", symbol: "doc.on.doc") {}
    private let pingButton = JourneyButton("⚡ Uplink", symbol: "bolt.fill") {}
    private let webButton = JourneyButton("🌐 Global", symbol: "safari") {}
    private let privacyCheckbox = NSButton(checkboxWithTitle: "Tactical Uplink", target: nil, action: nil)
    private let pingSpinner = NSProgressIndicator()
    private let radarCanvas = SwarmRadarCanvasView()

    init() {
        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 900, height: 600),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        win.title = "Sovereign Swarm Radar"
        win.minSize = NSSize(width: 760, height: 520)
        win.isReleasedWhenClosed = false
        super.init(window: win)
        win.delegate = self
        JourneyStyle.fitWindow(win, center: true)

        buildUI()
        setupListeners()
        updateUI(with: SwarmRadarManager.shared.currentState)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    func present() {
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func toggle() {
        if let win = window, win.isVisible && win.isKeyWindow {
            win.orderOut(nil)
        } else {
            present()
        }
    }

    private func buildUI() {
        guard let win = window else { return }

        let root = NSVisualEffectView()
        root.material = .hudWindow
        root.blendingMode = .behindWindow
        root.state = .active
        win.contentView = root

        pingSpinner.style = .spinning
        pingSpinner.controlSize = .small
        pingSpinner.isHidden = true

        nodeIdLabel.font = .monospacedSystemFont(ofSize: 11, weight: .bold)
        statsSummaryLabel.font = .monospacedSystemFont(ofSize: 11.5, weight: .semibold)
        statsSummaryLabel.textColor = .systemTeal

        copyButton.target = self; copyButton.action = #selector(copyNodeId); copyButton.bezelStyle = .inline; copyButton.controlSize = .small
        pingButton.target = self; pingButton.action = #selector(pingEdge); pingButton.controlSize = .small; pingButton.bezelColor = .controlAccentColor
        webButton.target = self; webButton.action = #selector(openWebRadar); webButton.controlSize = .small
        privacyCheckbox.target = self; privacyCheckbox.action = #selector(togglePrivacy); privacyCheckbox.controlSize = .small
        privacyCheckbox.state = SwarmRadarManager.shared.isHeartbeatDisabled ? .off : .on

        deployInterceptorBtn.target = self; deployInterceptorBtn.action = #selector(deployInterceptorAction); deployInterceptorBtn.controlSize = .small
        deployTurretBtn.target = self; deployTurretBtn.action = #selector(deployTurretAction); deployTurretBtn.controlSize = .small
        deployHeavyBtn.target = self; deployHeavyBtn.action = #selector(deployHeavyAction); deployHeavyBtn.controlSize = .small
        overdriveBtn.target = self; overdriveBtn.action = #selector(overdriveAction); overdriveBtn.controlSize = .small; overdriveBtn.bezelColor = .systemGreen
        orbitalStrikeBtn.target = self; orbitalStrikeBtn.action = #selector(orbitalStrikeAction); orbitalStrikeBtn.controlSize = .small; orbitalStrikeBtn.bezelColor = .systemOrange
        resetMissionBtn.target = self; resetMissionBtn.action = #selector(resetMissionAction); resetMissionBtn.controlSize = .small

        let topBar = JourneyStyle.stack([
            titleLabel,
            nodeIdLabel,
            copyButton,
            NSView(),
            statsSummaryLabel,
            statusPill
        ], vertical: false, spacing: 8)
        topBar.alignment = .centerY

        radarCanvas.setContentHuggingPriority(.init(1), for: .vertical)
        radarCanvas.setContentCompressionResistancePriority(.init(1), for: .vertical)
        radarCanvas.onStatsChanged = { [weak self] stats in
            guard let self else { return }
            self.deployInterceptorBtn.isEnabled = !stats.gameOver && stats.energy >= 25
            self.deployTurretBtn.isEnabled = !stats.gameOver && stats.energy >= 40
            self.deployHeavyBtn.isEnabled = !stats.gameOver && stats.energy >= 50
            self.overdriveBtn.isEnabled = !stats.gameOver && stats.energy >= 60
            self.orbitalStrikeBtn.isEnabled = !stats.gameOver && stats.energy >= 75
            self.statsSummaryLabel.stringValue = stats.gameOver
                ? "HQ BREACHED · FINAL SCORE \(stats.score)"
                : "WAVE \(stats.wave) · SCORE \(stats.score) · ⚡ \(stats.energy) · HULL \(stats.hull)%"
            self.statsSummaryLabel.textColor = stats.gameOver ? .systemRed : (stats.overdriveActive ? .systemGreen : .systemTeal)
        }

        let rtsDock = JourneyStyle.stack([
            deployInterceptorBtn,
            deployTurretBtn,
            deployHeavyBtn,
            overdriveBtn,
            orbitalStrikeBtn,
            resetMissionBtn,
            NSView(),
            privacyCheckbox,
            pingSpinner,
            pingButton,
            webButton
        ], vertical: false, spacing: 6)
        rtsDock.alignment = .centerY

        let mainStack = JourneyStyle.stack([
            topBar,
            radarCanvas,
            rtsDock
        ], spacing: 10)
        mainStack.edgeInsets = NSEdgeInsets(top: 12, left: 16, bottom: 12, right: 16)
        root.addSubview(mainStack)
        JourneyStyle.pin(mainStack, to: root)
        for view in mainStack.arrangedSubviews {
            view.widthAnchor.constraint(equalTo: mainStack.widthAnchor, constant: -32).isActive = true
        }
    }

    private func setupListeners() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleStateUpdate(_:)),
            name: .swarmRadarDidUpdate,
            object: nil
        )
    }

    @objc private func handleStateUpdate(_ notification: Notification) {
        if let state = notification.userInfo?["state"] as? SwarmRadarState {
            updateUI(with: state)
        } else {
            updateUI(with: SwarmRadarManager.shared.currentState)
        }
    }

    private func updateUI(with state: SwarmRadarState) {
        nodeIdLabel.stringValue = state.displayNodeId
        if state.isHeartbeatDisabled {
            statusPill.stringValue = "⚪ LOCAL SIM"
            statusPill.textColor = .secondaryLabelColor
            privacyCheckbox.state = .off
        } else {
            statusPill.stringValue = state.isOnline ? "🟢 UPLINK (\(state.latencyMs)ms)" : "🟡 SYNCING…"
            statusPill.textColor = state.isOnline ? .systemGreen : .systemYellow
            privacyCheckbox.state = .on
        }
        radarCanvas.update(
            isLive: !state.isHeartbeatDisabled && state.isOnline,
            activeUnits: state.activeNodes,
            sector: state.isHeartbeatDisabled ? "SECTOR-07 // LOCAL COMMAND" : state.region,
            callsign: state.displayNodeId
        )
    }

    @objc private func copyNodeId() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(SwarmRadarManager.shared.currentState.displayNodeId, forType: .string)
        copyButton.title = "Copied!"
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
            self?.copyButton.title = "Copy"
        }
    }

    @objc private func pingEdge() {
        pingSpinner.isHidden = false
        pingSpinner.startAnimation(nil)
        pingButton.isEnabled = false
        SwarmRadarManager.shared.ping(action: "Edge Latency Ping", source: "radar_inspector") { [weak self] _ in
            self?.pingSpinner.stopAnimation(nil)
            self?.pingSpinner.isHidden = true
            self?.pingButton.isEnabled = true
        }
    }

    @objc private func openWebRadar() {
        if let url = URL(string: "https://heav3ns.com/code#swarm-radar") {
            NSWorkspace.shared.open(url)
        }
    }

    @objc private func deployInterceptorAction() { _ = radarCanvas.deployInterceptor() }
    @objc private func deployTurretAction() { _ = radarCanvas.deployTurret() }
    @objc private func deployHeavyAction() { _ = radarCanvas.deployHeavyMech() }
    @objc private func overdriveAction() { _ = radarCanvas.triggerOverdrive() }
    @objc private func orbitalStrikeAction() { _ = radarCanvas.triggerOrbitalStrike() }
    @objc private func resetMissionAction() { radarCanvas.resetMission() }

    @objc private func togglePrivacy() {
        SwarmRadarManager.shared.setHeartbeatDisabled(privacyCheckbox.state == .off)
    }

    func dispose() {
        radarCanvas.dispose()
        NotificationCenter.default.removeObserver(self)
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }
}
