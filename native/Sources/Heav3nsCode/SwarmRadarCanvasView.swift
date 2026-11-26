// [LAYER: PLUMBING]
import AppKit

/// Full-bleed interactive Mecha/RTS Orbital Conquest game simulator.
/// Left-click to rally your squadron; use hotkeys (1-5, Space, R) or the command dock
/// to deploy Fighters, Heavy Gundams, Defense Turrets, Overdrive Shields, and Orbital Strikes.
final class SwarmRadarCanvasView: NSView {
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    struct Unit {
        var pos: CGPoint
        var target: CGPoint
        var heading: CGFloat
        var hp: CGFloat
        var maxHp: CGFloat
        var speed: CGFloat
        var damage: CGFloat
        var cooldown: CGFloat
        var isHeavy: Bool
        var isTurret: Bool
    }

    struct Enemy {
        var pos: CGPoint
        var hp: CGFloat
        var maxHp: CGFloat
        var speed: CGFloat
        var isBoss: Bool
    }

    struct Beam {
        var from: CGPoint
        var to: CGPoint
        var color: NSColor
        var ttl: CGFloat
    }

    struct Burst {
        var pos: CGPoint
        var radius: CGFloat
        var maxRadius: CGFloat
        var color: NSColor
        var ttl: CGFloat
    }

    struct FloatingTag {
        var pos: CGPoint
        var text: String
        var color: NSColor
        var ttl: CGFloat
    }

    struct Stats {
        let wave: Int
        let score: Int
        let energy: Int
        let hull: Int
        let unitCount: Int
        let enemyCount: Int
        let overdriveActive: Bool
        let gameOver: Bool
    }

    var onStatsChanged: ((Stats) -> Void)?

    private var sweepAngle: CGFloat = 0
    private var tickTimer: Timer?
    private var units = [Unit]()
    private var enemies = [Enemy]()
    private var beams = [Beam]()
    private var bursts = [Burst]()
    private var tags = [FloatingTag]()
    private var stars: [(CGPoint, CGFloat)] = []
    private var rallyPoint: CGPoint?
    private var wave = 1
    private var score = 0
    private var energy: CGFloat = 75
    private var hull: CGFloat = 100
    private var overdriveRemaining: CGFloat = 0
    private var spawnAccumulator: CGFloat = 0
    private var waveRemaining = 5
    private var gameOver = false
    private var sectorTitle = "SECTOR-07 // ORBITAL COMMAND"
    private var callsignTitle = "NODE_LOCAL"

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.cornerRadius = 12
        layer?.borderWidth = 1.25
        layer?.borderColor = NSColor.systemTeal.withAlphaComponent(0.55).cgColor
        stars = (0..<48).map { _ in
            (CGPoint(x: CGFloat.random(in: 0.02...0.98), y: CGFloat.random(in: 0.02...0.98)), CGFloat.random(in: 0.2...0.65))
        }
        resetMission()
        startLoop()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    func update(isLive: Bool, activeUnits: Int?, sector: String, callsign: String) {
        self.sectorTitle = sector
        self.callsignTitle = callsign
        needsDisplay = true
    }

    func resetMission() {
        wave = 1
        score = 0
        energy = 75
        hull = 100
        overdriveRemaining = 0
        gameOver = false
        waveRemaining = 5
        spawnAccumulator = 1.0
        rallyPoint = nil
        beams.removeAll()
        bursts.removeAll()
        tags.removeAll()
        enemies.removeAll()
        units = [
            Unit(pos: CGPoint(x: 0.42, y: 0.42), target: CGPoint(x: 0.42, y: 0.42), heading: -0.6, hp: 50, maxHp: 50, speed: 0.24, damage: 13, cooldown: 0, isHeavy: false, isTurret: false),
            Unit(pos: CGPoint(x: 0.58, y: 0.42), target: CGPoint(x: 0.58, y: 0.42), heading: -2.5, hp: 50, maxHp: 50, speed: 0.24, damage: 13, cooldown: 0, isHeavy: false, isTurret: false),
            Unit(pos: CGPoint(x: 0.50, y: 0.62), target: CGPoint(x: 0.50, y: 0.62), heading: 1.5, hp: 100, maxHp: 100, speed: 0.14, damage: 26, cooldown: 0, isHeavy: true, isTurret: false)
        ]
        notifyStats()
        needsDisplay = true
    }

    @discardableResult
    func deployInterceptor() -> Bool {
        guard !gameOver, energy >= 25 else { return false }
        energy -= 25
        let angle = CGFloat.random(in: 0..<(2 * .pi))
        let spawn = CGPoint(x: 0.5 + cos(angle) * 0.08, y: 0.5 + sin(angle) * 0.08)
        let tgt = rallyPoint ?? CGPoint(x: 0.5 + cos(angle) * 0.24, y: 0.5 + sin(angle) * 0.24)
        units.append(Unit(pos: spawn, target: tgt, heading: angle, hp: 55, maxHp: 55, speed: 0.25, damage: 14, cooldown: 0, isHeavy: false, isTurret: false))
        bursts.append(Burst(pos: spawn, radius: 3, maxRadius: 22, color: .systemTeal, ttl: 0.35))
        notifyStats()
        return true
    }

    @discardableResult
    func deployTurret() -> Bool {
        let turretCount = units.filter(\.isTurret).count
        guard !gameOver, energy >= 40, turretCount < 4 else { return false }
        energy -= 40
        let pos = rallyPoint ?? CGPoint(x: 0.5 + CGFloat.random(in: -0.18...0.18), y: 0.5 + CGFloat.random(in: -0.18...0.18))
        units.append(Unit(pos: pos, target: pos, heading: 0, hp: 120, maxHp: 120, speed: 0, damage: 18, cooldown: 0, isHeavy: false, isTurret: true))
        bursts.append(Burst(pos: pos, radius: 4, maxRadius: 26, color: .systemCyan, ttl: 0.4))
        tags.append(FloatingTag(pos: pos, text: "TURRET ONLINE", color: .systemCyan, ttl: 0.9))
        notifyStats()
        return true
    }

    @discardableResult
    func deployHeavyMech() -> Bool {
        guard !gameOver, energy >= 50 else { return false }
        energy -= 50
        let angle = CGFloat.random(in: 0..<(2 * .pi))
        let spawn = CGPoint(x: 0.5 + cos(angle) * 0.09, y: 0.5 + sin(angle) * 0.09)
        let tgt = rallyPoint ?? CGPoint(x: 0.5 + cos(angle) * 0.18, y: 0.5 + sin(angle) * 0.18)
        units.append(Unit(pos: spawn, target: tgt, heading: angle, hp: 120, maxHp: 120, speed: 0.14, damage: 30, cooldown: 0, isHeavy: true, isTurret: false))
        bursts.append(Burst(pos: spawn, radius: 5, maxRadius: 32, color: .systemGreen, ttl: 0.45))
        notifyStats()
        return true
    }

    @discardableResult
    func triggerOverdrive() -> Bool {
        guard !gameOver, energy >= 60 else { return false }
        energy -= 60
        hull = min(100, hull + 30)
        overdriveRemaining = 4.5
        bursts.append(Burst(pos: CGPoint(x: 0.5, y: 0.5), radius: 10, maxRadius: 85, color: .systemGreen, ttl: 0.6))
        tags.append(FloatingTag(pos: CGPoint(x: 0.5, y: 0.42), text: "OVERDRIVE +30% HULL", color: .systemGreen, ttl: 1.1))
        notifyStats()
        return true
    }

    @discardableResult
    func triggerOrbitalStrike() -> Bool {
        guard !gameOver, energy >= 75 else { return false }
        energy -= 75
        let targetPos = rallyPoint ?? enemies.first?.pos ?? CGPoint(x: 0.5, y: 0.28)
        bursts.append(Burst(pos: targetPos, radius: 8, maxRadius: 90, color: .systemYellow, ttl: 0.7))
        tags.append(FloatingTag(pos: targetPos, text: "ORBITAL STRIKE", color: .systemYellow, ttl: 1.0))
        var survivors = [Enemy]()
        for var e in enemies {
            let d = hypot(e.pos.x - targetPos.x, e.pos.y - targetPos.y)
            if d < 0.28 {
                e.hp -= 95
                if e.hp <= 0 {
                    let pts = e.isBoss ? 120 : 30
                    score += pts
                    energy = min(150, energy + 8)
                    bursts.append(Burst(pos: e.pos, radius: 3, maxRadius: 28, color: .systemOrange, ttl: 0.4))
                    tags.append(FloatingTag(pos: e.pos, text: "+\(pts)", color: .systemOrange, ttl: 0.8))
                    continue
                }
            }
            survivors.append(e)
        }
        enemies = survivors
        notifyStats()
        return true
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let loc = convert(event.locationInWindow, from: nil)
        guard bounds.width > 10, bounds.height > 10 else { return }
        if gameOver {
            resetMission()
            return
        }
        let nx = max(0.05, min(0.95, loc.x / bounds.width))
        let ny = max(0.07, min(0.93, loc.y / bounds.height))
        let target = CGPoint(x: nx, y: ny)
        rallyPoint = target
        var mobileIdx = 0
        for idx in units.indices where !units[idx].isTurret {
            let spreadAngle = CGFloat(mobileIdx) * 2.1
            let r: CGFloat = 0.045
            units[idx].target = CGPoint(
                x: max(0.05, min(0.95, nx + cos(spreadAngle) * r)),
                y: max(0.07, min(0.93, ny + sin(spreadAngle) * r))
            )
            mobileIdx += 1
        }
        bursts.append(Burst(pos: target, radius: 2, maxRadius: 24, color: .systemTeal, ttl: 0.3))
        needsDisplay = true
    }

    override func keyDown(with event: NSEvent) {
        switch event.charactersIgnoringModifiers?.lowercased() {
        case "1": _ = deployInterceptor()
        case "2": _ = deployTurret()
        case "3": _ = deployHeavyMech()
        case "4": _ = triggerOverdrive()
        case "5", " ": _ = triggerOrbitalStrike()
        case "r": resetMission()
        default: super.keyDown(with: event)
        }
    }

    private func startLoop() {
        guard tickTimer == nil else { return }
        tickTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            guard let self else { return }
            MainActor.assumeIsolated {
                self.stepSimulation(dt: 1.0 / 30.0)
            }
        }
    }

    private func stepSimulation(dt: CGFloat) {
        sweepAngle += overdriveRemaining > 0 ? 0.08 : 0.045
        if sweepAngle > .pi * 2 { sweepAngle -= .pi * 2 }
        overdriveRemaining = max(0, overdriveRemaining - dt)

        beams = beams.compactMap { var b = $0; b.ttl -= dt; return b.ttl > 0 ? b : nil }
        bursts = bursts.compactMap {
            var b = $0
            b.ttl -= dt
            b.radius = min(b.maxRadius, b.radius + dt * 110)
            return b.ttl > 0 ? b : nil
        }
        tags = tags.compactMap {
            var t = $0
            t.ttl -= dt
            t.pos.y -= dt * 0.03
            return t.ttl > 0 ? t : nil
        }

        guard !gameOver else {
            needsDisplay = true
            return
        }

        energy = min(150, energy + dt * 12.0)

        spawnAccumulator -= dt
        if waveRemaining > 0 && spawnAccumulator <= 0 {
            spawnAccumulator = max(0.55, 1.9 - CGFloat(wave) * 0.13)
            waveRemaining -= 1
            let edgeAngle = CGFloat.random(in: 0..<(2 * .pi))
            let spawn = CGPoint(x: 0.5 + cos(edgeAngle) * 0.47, y: 0.5 + sin(edgeAngle) * 0.47)
            let isBoss = waveRemaining == 0 && wave % 2 == 0
            let hp: CGFloat = isBoss ? CGFloat(100 + wave * 24) : CGFloat(24 + wave * 7)
            enemies.append(Enemy(pos: spawn, hp: hp, maxHp: hp, speed: isBoss ? 0.045 : 0.075, isBoss: isBoss))
        } else if waveRemaining == 0 && enemies.isEmpty {
            wave += 1
            waveRemaining = 4 + wave * 2
            energy = min(150, energy + 25)
            bursts.append(Burst(pos: CGPoint(x: 0.5, y: 0.5), radius: 8, maxRadius: 70, color: .systemGreen, ttl: 0.65))
            tags.append(FloatingTag(pos: CGPoint(x: 0.5, y: 0.42), text: "WAVE \(wave)", color: .systemGreen, ttl: 1.2))
        }

        let fireRateMult: CGFloat = overdriveRemaining > 0 ? 0.5 : 1.0
        for i in units.indices {
            units[i].cooldown = max(0, units[i].cooldown - dt)
            if !units[i].isTurret {
                let dx = units[i].target.x - units[i].pos.x
                let dy = units[i].target.y - units[i].pos.y
                let dist = hypot(dx, dy)
                if dist > 0.01 {
                    units[i].heading = atan2(dy, dx)
                    let step = min(dist, units[i].speed * dt)
                    units[i].pos.x += (dx / dist) * step
                    units[i].pos.y += (dy / dist) * step
                }
            }
            let range: CGFloat = units[i].isTurret ? 0.34 : 0.30
            if units[i].cooldown <= 0, let targetIdx = nearestEnemyIndex(from: units[i].pos, maxRange: range) {
                units[i].heading = atan2(enemies[targetIdx].pos.y - units[i].pos.y, enemies[targetIdx].pos.x - units[i].pos.x)
                let baseCd: CGFloat = units[i].isHeavy ? 0.72 : (units[i].isTurret ? 0.52 : 0.42)
                units[i].cooldown = baseCd * fireRateMult
                enemies[targetIdx].hp -= units[i].damage
                let beamColor: NSColor = overdriveRemaining > 0 ? .systemYellow : (units[i].isHeavy ? .systemGreen : .systemTeal)
                beams.append(Beam(from: units[i].pos, to: enemies[targetIdx].pos, color: beamColor, ttl: 0.16))
            }
        }

        var nextEnemies = [Enemy]()
        for e in enemies {
            if e.hp <= 0 {
                let pts = e.isBoss ? 120 : 25
                score += pts
                energy = min(150, energy + 7)
                bursts.append(Burst(pos: e.pos, radius: 3, maxRadius: e.isBoss ? 38 : 22, color: .systemOrange, ttl: 0.35))
                tags.append(FloatingTag(pos: e.pos, text: e.isBoss ? "BOSS +\(pts)" : "+\(pts)", color: e.isBoss ? .systemYellow : .systemOrange, ttl: 0.75))
            } else {
                nextEnemies.append(e)
            }
        }
        enemies = nextEnemies

        for i in enemies.indices {
            let ePos = enemies[i].pos
            let dx = 0.5 - ePos.x
            let dy = 0.5 - ePos.y
            let dist = hypot(dx, dy)
            if dist > 0.04 {
                enemies[i].pos.x += (dx / dist) * enemies[i].speed * dt
                enemies[i].pos.y += (dy / dist) * enemies[i].speed * dt
            } else {
                hull = max(0, hull - dt * (enemies[i].isBoss ? 14 : 7))
                if hull <= 0 { gameOver = true }
            }
            if let uIdx = nearestUnitIndex(from: enemies[i].pos, maxRange: 0.11) {
                units[uIdx].hp -= dt * (enemies[i].isBoss ? 18 : 9)
            }
        }
        units.removeAll { $0.hp <= 0 }

        notifyStats()
        needsDisplay = true
    }

    private func nearestEnemyIndex(from pt: CGPoint, maxRange: CGFloat) -> Int? {
        var bestIdx: Int?
        var bestDist = maxRange
        for (idx, e) in enemies.enumerated() {
            let d = hypot(e.pos.x - pt.x, e.pos.y - pt.y)
            if d < bestDist { bestDist = d; bestIdx = idx }
        }
        return bestIdx
    }

    private func nearestUnitIndex(from pt: CGPoint, maxRange: CGFloat) -> Int? {
        var bestIdx: Int?
        var bestDist = maxRange
        for (idx, u) in units.enumerated() {
            let d = hypot(u.pos.x - pt.x, u.pos.y - pt.y)
            if d < bestDist { bestDist = d; bestIdx = idx }
        }
        return bestIdx
    }

    private func notifyStats() {
        onStatsChanged?(Stats(
            wave: wave,
            score: score,
            energy: Int(energy),
            hull: Int(hull),
            unitCount: units.count,
            enemyCount: enemies.count,
            overdriveActive: overdriveRemaining > 0,
            gameOver: gameOver
        ))
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor(calibratedRed: 0.035, green: 0.06, blue: 0.095, alpha: 0.98).setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 12, yRadius: 12).fill()

        let w = bounds.width, h = bounds.height
        let center = NSPoint(x: w / 2, y: h / 2)
        let maxR = max(50, min(w, h) * 0.42)

        // Subtle starfield
        for (pt, alpha) in stars {
            NSColor.white.withAlphaComponent(alpha * 0.35).setFill()
            NSBezierPath(ovalIn: NSRect(x: pt.x * w, y: pt.y * h, width: 1.5, height: 1.5)).fill()
        }

        // Tactical grid
        NSColor.systemTeal.withAlphaComponent(0.075).setStroke()
        let grid = NSBezierPath(); grid.lineWidth = 0.75
        var gx: CGFloat = 32; while gx < w { grid.move(to: NSPoint(x: gx, y: 0)); grid.line(to: NSPoint(x: gx, y: h)); gx += 32 }
        var gy: CGFloat = 32; while gy < h { grid.move(to: NSPoint(x: 0, y: gy)); grid.line(to: NSPoint(x: w, y: gy)); gy += 32 }
        grid.stroke()

        // Range rings
        for factor: CGFloat in [0.26, 0.52, 0.78, 1.0] {
            let r = maxR * factor
            let ring = NSBezierPath(ovalIn: NSRect(x: center.x - r, y: center.y - r, width: r * 2, height: r * 2))
            ring.lineWidth = factor == 1.0 ? 1.15 : 0.8
            NSColor.systemTeal.withAlphaComponent(factor == 1.0 ? 0.34 : 0.16).setStroke()
            ring.stroke()
        }

        // Sweep beam
        let beam = NSBezierPath()
        beam.move(to: center)
        beam.line(to: NSPoint(x: center.x + cos(sweepAngle) * maxR, y: center.y + sin(sweepAngle) * maxR))
        beam.lineWidth = 1.4
        (overdriveRemaining > 0 ? NSColor.systemGreen : NSColor.systemTeal).withAlphaComponent(0.32).setStroke()
        beam.stroke()

        // Rally reticle
        if let rp = rallyPoint {
            let p = NSPoint(x: rp.x * w, y: rp.y * h)
            let reticle = NSBezierPath(ovalIn: NSRect(x: p.x - 8, y: p.y - 8, width: 16, height: 16))
            NSColor.systemTeal.withAlphaComponent(0.65).setStroke()
            reticle.lineWidth = 1.2
            reticle.stroke()
        }

        // Laser beams
        for b in beams {
            let path = NSBezierPath()
            path.move(to: NSPoint(x: b.from.x * w, y: b.from.y * h))
            path.line(to: NSPoint(x: b.to.x * w, y: b.to.y * h))
            path.lineWidth = 2.0
            b.color.withAlphaComponent(min(1, b.ttl * 6)).setStroke()
            path.stroke()
        }

        // Shockwaves
        for b in bursts {
            let p = NSPoint(x: b.pos.x * w, y: b.pos.y * h)
            let ring = NSBezierPath(ovalIn: NSRect(x: p.x - b.radius, y: p.y - b.radius, width: b.radius * 2, height: b.radius * 2))
            b.color.withAlphaComponent(min(1, b.ttl * 2.2)).setStroke()
            ring.lineWidth = 2.2
            ring.stroke()
        }

        // Enemies + HP bars
        for e in enemies {
            let p = NSPoint(x: e.pos.x * w, y: e.pos.y * h)
            let r: CGFloat = e.isBoss ? 8.0 : 5.0
            let diamond = NSBezierPath()
            diamond.move(to: NSPoint(x: p.x, y: p.y - r))
            diamond.line(to: NSPoint(x: p.x + r, y: p.y))
            diamond.line(to: NSPoint(x: p.x, y: p.y + r))
            diamond.line(to: NSPoint(x: p.x - r, y: p.y))
            diamond.close()
            (e.isBoss ? NSColor.systemRed : NSColor.systemOrange).setFill()
            diamond.fill()
            if e.hp < e.maxHp {
                drawHPBar(at: NSPoint(x: p.x, y: p.y - r - 5), ratio: e.hp / e.maxHp, color: .systemRed, width: e.isBoss ? 22 : 14)
            }
        }

        // Friendly Mecha Units (directional triangles / heavy diamonds / turrets) + HP bars
        for u in units {
            let p = NSPoint(x: u.pos.x * w, y: u.pos.y * h)
            if u.isTurret {
                let box = NSBezierPath(roundedRect: NSRect(x: p.x - 5, y: p.y - 5, width: 10, height: 10), xRadius: 2, yRadius: 2)
                NSColor.systemCyan.setFill()
                box.fill()
            } else {
                let size: CGFloat = u.isHeavy ? 7.5 : 5.5
                let tri = NSBezierPath()
                tri.move(to: NSPoint(x: p.x + cos(u.heading) * size, y: p.y + sin(u.heading) * size))
                tri.line(to: NSPoint(x: p.x + cos(u.heading + 2.4) * size, y: p.y + sin(u.heading + 2.4) * size))
                tri.line(to: NSPoint(x: p.x + cos(u.heading - 2.4) * size, y: p.y + sin(u.heading - 2.4) * size))
                tri.close()
                (u.isHeavy ? NSColor.systemGreen : NSColor.systemTeal).setFill()
                tri.fill()
            }
            if u.hp < u.maxHp {
                drawHPBar(at: NSPoint(x: p.x, y: p.y - 10), ratio: u.hp / u.maxHp, color: .systemGreen, width: 16)
            }
        }

        // Center HQ + Shield Dome
        let shieldR: CGFloat = 18
        let shield = NSBezierPath(ovalIn: NSRect(x: center.x - shieldR, y: center.y - shieldR, width: shieldR * 2, height: shieldR * 2))
        (overdriveRemaining > 0 ? NSColor.systemGreen : NSColor.systemTeal).withAlphaComponent(max(0.15, hull / 100 * 0.55)).setStroke()
        shield.lineWidth = overdriveRemaining > 0 ? 2.2 : 1.3
        shield.stroke()
        let hq = NSBezierPath(ovalIn: NSRect(x: center.x - 7, y: center.y - 7, width: 14, height: 14))
        (hull > 35 ? NSColor.systemTeal : NSColor.systemRed).setFill()
        hq.fill()

        // Floating combat tags
        let tagFont = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .bold)
        for t in tags {
            let p = NSPoint(x: t.pos.x * w - 14, y: t.pos.y * h - 10)
            (t.text as NSString).draw(at: p, withAttributes: [.font: tagFont, .foregroundColor: t.color.withAlphaComponent(min(1, t.ttl * 1.6))])
        }

        // Top & Bottom HUD overlays
        let hudFont = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .semibold)
        let hudText = "WAVE \(wave)  ·  SCORE \(score)  ·  ⚡ \(Int(energy))/150  ·  HULL \(Int(hull))%  ·  UNITS \(units.count)"
        (hudText as NSString).draw(at: NSPoint(x: 16, y: 14), withAttributes: [.font: hudFont, .foregroundColor: NSColor.systemTeal])
        let hint = gameOver ? "HQ BREACHED — CLICK GRID OR PRESS R TO RELAUNCH" : "\(sectorTitle) · CLICK TO RALLY · HOTKEYS [1-5] [SPACE] [R]"
        (hint as NSString).draw(at: NSPoint(x: 16, y: h - 24), withAttributes: [.font: hudFont, .foregroundColor: (gameOver ? NSColor.systemRed : NSColor.systemGreen).withAlphaComponent(0.9)])
        (callsignTitle as NSString).draw(at: NSPoint(x: center.x + 12, y: center.y - 16), withAttributes: [.font: NSFont.monospacedSystemFont(ofSize: 9.5, weight: .bold), .foregroundColor: NSColor.white.withAlphaComponent(0.9)])
    }

    private func drawHPBar(at centerTop: NSPoint, ratio: CGFloat, color: NSColor, width: CGFloat) {
        let clamped = max(0, min(1, ratio))
        NSColor.black.withAlphaComponent(0.55).setFill()
        NSBezierPath(rect: NSRect(x: centerTop.x - width / 2, y: centerTop.y, width: width, height: 2.5)).fill()
        color.setFill()
        NSBezierPath(rect: NSRect(x: centerTop.x - width / 2, y: centerTop.y, width: width * clamped, height: 2.5)).fill()
    }

    func dispose() {
        tickTimer?.invalidate()
        tickTimer = nil
        onStatsChanged = nil
    }

    deinit {
        tickTimer?.invalidate()
    }
}
