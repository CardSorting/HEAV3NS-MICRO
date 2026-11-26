#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * GALXAI Documentation Sovereignty — Atomic Autoroller (Zenith Edition).
 *
 * Synchronizes ROADMAP.md + docs/roadmap/README.md from docs/roadmap/{now,next,later,discovery,decision-log,archive}.md
 * and CHANGELOG.md + docs/changelog/README.md from docs/changelog/*.md.
 *
 * Zenith Hardening over v1:
 *  - Midnight-safe date stability: preserves existing `Synced YYYY-MM-DD` in `--check` (and no-op rolls) so CI never flakes at 00:00 UTC.
 *  - Separates `invariantErrors` from `staleFiles` for unambiguous machine-readable `--json` diagnostics.
 *  - Rejects duplicate/unbalanced `<!-- AUTOROLL:START/END -->` markers fail-closed.
 *  - Atomic `StagedFs` write-temp + rename + rollback + stale `.rolltmp-*` cleanup on failure.
 */
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const argv = process.argv.slice(2);
if (argv.includes("--help") || argv.includes("-h")) {
  console.log(`Usage: node bin/docs-roll.mjs [--check] [--dry-run] [--json] [--root <dir>] [--complete <match>] [--release <semver>] [--date YYYY-MM-DD]`);
  process.exit(0);
}
function flagValue(name) {
  const idx = argv.indexOf(name);
  return idx >= 0 && idx + 1 < argv.length && !argv[idx + 1].startsWith("--") ? argv[idx + 1] : "";
}
const hasFlag = (name) => argv.includes(name);

const checkOnly = hasFlag("--check");
const dryRun = hasFlag("--dry-run");
const jsonOutput = hasFlag("--json");
const promote = hasFlag("--promote") || !checkOnly;
const completeTarget = flagValue("--complete").trim();
const addRoadmapTitle = flagValue("--add-roadmap").trim();
const addRoadmapHorizon = (flagValue("--horizon").trim() || "next").toLowerCase();
const addRoadmapGoal = flagValue("--goal").trim();
const addRoadmapStatus = flagValue("--status").trim();
const addDecisionTitle = flagValue("--add-decision").trim();
const addDecisionText = flagValue("--decision").trim();
const addDecisionReason = flagValue("--reason").trim();
const addDecisionImpact = flagValue("--impact").trim();
const addChangelogTitle = flagValue("--add-changelog").trim();
const addChangelogSection = flagValue("--section").trim() || "Changed";
const addChangelogSummary = flagValue("--summary").trim();
const addChangelogRefs = flagValue("--refs").split(",").map((s) => s.trim()).filter(Boolean);
const fromGitRef = hasFlag("--from-git") ? (flagValue("--from-git") || "HEAD~5") : "";
const releaseVersion = flagValue("--release").replace(/^v/, "").trim();
const maxUnreleased = Math.max(3, Number(flagValue("--max-unreleased") || "12") || 12);
const explicitDate = flagValue("--date").trim();
const today = explicitDate || new Date().toISOString().slice(0, 10);
const root = path.resolve(flagValue("--root") || process.cwd());

const AUTOROLL_START = "<!-- AUTOROLL:START -->";
const AUTOROLL_END = "<!-- AUTOROLL:END -->";
const MAX_ROOT_LINES = 160;
const NOW_WIP_TARGET = 3;
const NEXT_WIP_TARGET = 3;
const VALID_STATUSES = new Set(["active", "ready", "planned", "investigating", "blocked", "completed", "done", "rolled"]);

const HORIZONS = [
  { key: "now", file: "now.md", title: "Now", desc: "Immediate active engineering & verification priorities", defaultStatus: "Active" },
  { key: "next", file: "next.md", title: "Next", desc: "Upcoming queued work ready for promotion", defaultStatus: "Ready" },
  { key: "later", file: "later.md", title: "Later", desc: "Longer-term architecture & scale investments", defaultStatus: "Planned" },
  { key: "discovery", file: "discovery.md", title: "Discovery", desc: "Open technical investigations & questions", defaultStatus: "Investigating" },
];

const cleanTitle = (raw) => raw.replace(/^\d+\.\s*/, "").trim();

class StagedFs {
  constructor() {
    this.staged = new Map();
  }
  read(rel) {
    if (this.staged.has(rel)) return this.staged.get(rel);
    return fs.readFileSync(path.join(root, rel), "utf8");
  }
  exists(rel) {
    return this.staged.has(rel) || fs.existsSync(path.join(root, rel));
  }
  stage(rel, nextContent, staleFiles) {
    const prev = this.exists(rel) ? this.read(rel) : "";
    if (prev === nextContent) return false;
    this.staged.set(rel, nextContent);
    if (!staleFiles.includes(rel)) staleFiles.push(rel);
    return true;
  }
  commitAtomic() {
    const backups = [];
    const temps = [];
    try {
      for (const [rel, content] of this.staged.entries()) {
        const full = path.join(root, rel);
        fs.mkdirSync(path.dirname(full), { recursive: true });
        const existed = fs.existsSync(full);
        const prev = existed ? fs.readFileSync(full) : null;
        const tmp = `${full}.rolltmp-${process.pid}-${Date.now()}`;
        temps.push(tmp);
        fs.writeFileSync(tmp, content, "utf8");
        fs.renameSync(tmp, full);
        backups.push({ full, existed, prev });
      }
    } catch (err) {
      for (const t of temps) {
        try { if (fs.existsSync(t)) fs.unlinkSync(t); } catch { /* best-effort */ }
      }
      for (const b of backups.reverse()) {
        try {
          if (b.existed) fs.writeFileSync(b.full, b.prev);
          else if (fs.existsSync(b.full)) fs.unlinkSync(b.full);
        } catch { /* best-effort */ }
      }
      throw err;
    }
  }
}

function replaceAutorollBlock(content, replacement, rel) {
  const starts = (content.match(/<!-- AUTOROLL:START -->/g) || []).length;
  const ends = (content.match(/<!-- AUTOROLL:END -->/g) || []).length;
  const s = content.indexOf(AUTOROLL_START);
  const e = content.indexOf(AUTOROLL_END);
  if (starts !== 1 || ends !== 1 || s < 0 || e < 0 || e < s) {
    throw new Error(`${rel} must have exactly one balanced ${AUTOROLL_START} / ${AUTOROLL_END} pair`);
  }
  return `${content.slice(0, s + AUTOROLL_START.length)}\n${replacement.trim()}\n${content.slice(e)}`;
}

function splitSections(md, headingPrefix) {
  const lines = md.split(/\r?\n/);
  const items = [];
  const prefix = [];
  let cur = null;
  for (const line of lines) {
    if (line.startsWith(headingPrefix)) {
      if (cur) items.push(cur);
      cur = { title: line.slice(headingPrefix.length).trim(), lines: [line] };
    } else if (cur) {
      cur.lines.push(line);
    } else {
      prefix.push(line);
    }
  }
  if (cur) items.push(cur);
  return { prefix: prefix.join("\n").trimEnd(), items };
}

function itemField(item, fieldName) {
  const m = item.lines.join("\n").match(new RegExp(`^-\\s*\\*\\*${fieldName}:\\*\\*\\s*(.+)$`, "im"));
  return m ? m[1].trim() : "";
}

function setItemStatus(item, nextStatus) {
  let replaced = false;
  item.lines = item.lines.map((l) => {
    if (/^-\s*\*\*Status:\*\*/i.test(l)) {
      replaced = true;
      return `- **Status:** ${nextStatus}`;
    }
    return l;
  });
  if (!replaced) item.lines.splice(1, 0, `- **Status:** ${nextStatus}`);
}

const isCompletedItem = (item) => {
  const st = itemField(item, "Status").toLowerCase();
  return st === "completed" || st === "done" || st === "rolled" || item.lines.some((l) => /^-\s*\[[xX]\]\s+/.test(l));
};

function rewriteRelativeLinks(text, fromDirRel, toDirRel) {
  return text.replace(/(\[[^\]]*\]\()([^)\n]+)(\))/g, (m, p1, target, p3) => {
    const t = target.trim();
    if (!t || t.startsWith("#") || /^(https?:|mailto:|tel:|data:)/i.test(t)) return m;
    const [pathname, suffix = ""] = t.split(/([?#].*)/, 2);
    const resolved = path.resolve(root, fromDirRel, pathname);
    let rel = path.relative(path.join(root, toDirRel), resolved).split(path.sep).join("/");
    if (!rel) rel = ".";
    return `${p1}${rel}${suffix}${p3}`;
  });
}

function formatH3Items(prefix, items) {
  const body = items.map((it) => it.lines.join("\n").trim()).filter(Boolean).join("\n\n");
  return body ? `${prefix}\n\n${body}\n` : `${prefix}\n`;
}

function renumberItems(items) {
  return items.map((it, idx) => {
    const t = `${idx + 1}. ${cleanTitle(it.title)}`;
    return { title: t, lines: [`### ${t}`, ...it.lines.slice(1)] };
  });
}

function rollRoadmap(sfs, staleFiles, invariantErrors) {
  const parsed = {};
  const completed = [];
  const seenTitles = new Map();
  let matchedComplete = !completeTarget;

  if (!checkOnly && addRoadmapTitle) {
    const targetH = HORIZONS.find((h) => h.key === addRoadmapHorizon);
    if (!targetH) throw new Error(`Invalid --horizon '${addRoadmapHorizon}' (expected now|next|later|discovery)`);
    const rel = `docs/roadmap/${targetH.file}`;
    const sec = splitSections(sfs.read(rel), "### ");
    const normNew = cleanTitle(addRoadmapTitle).toLowerCase();
    if (!sec.items.some((it) => cleanTitle(it.title).toLowerCase() === normNew)) {
      const label = targetH.key === "discovery" ? "Question" : "Goal";
      const status = addRoadmapStatus || targetH.defaultStatus;
      const lines = [`### 999. ${cleanTitle(addRoadmapTitle)}`, "", `- **Status:** ${status}`];
      if (addRoadmapGoal) lines.push(`- **${label}:** ${addRoadmapGoal}`);
      sec.items.push({ title: `999. ${cleanTitle(addRoadmapTitle)}`, lines });
      sfs.stage(rel, formatH3Items(sec.prefix, renumberItems(sec.items)), staleFiles);
    }
  }

  if (!checkOnly && addDecisionTitle) {
    const decRel = "docs/roadmap/decision-log.md";
    const decSec = splitSections(sfs.read(decRel), "### ");
    const decLines = [`### ${today} — ${addDecisionTitle}`, ""];
    if (addDecisionText) decLines.push(`- **Decision:** ${addDecisionText}`);
    if (addDecisionReason) decLines.push(`- **Reason:** ${addDecisionReason}`);
    if (addDecisionImpact) decLines.push(`- **Impact:** ${addDecisionImpact}`);
    decSec.items.unshift({ title: `${today} — ${addDecisionTitle}`, lines: decLines });
    sfs.stage(decRel, formatH3Items(decSec.prefix, decSec.items), staleFiles);
  }

  for (const h of HORIZONS) {
    const rel = `docs/roadmap/${h.file}`;
    const raw = sfs.read(rel);
    if (!raw.startsWith("<!-- [LAYER: UI] -->")) invariantErrors.push(`${rel} missing <!-- [LAYER: UI] --> header`);
    const sec = splitSections(raw, "### ");
    const active = [];
    for (const item of sec.items) {
      const cleaned = cleanTitle(item.title);
      const norm = cleaned.toLowerCase();
      if (seenTitles.has(norm)) invariantErrors.push(`Duplicate roadmap item '${cleaned}' in ${rel} and ${seenTitles.get(norm)}`);
      else seenTitles.set(norm, rel);

      const st = itemField(item, "Status").toLowerCase();
      if (st && !VALID_STATUSES.has(st)) invariantErrors.push(`Invalid status '${st}' on '${cleaned}' in ${rel}`);
      if (completeTarget && norm.includes(completeTarget.toLowerCase())) {
        setItemStatus(item, "Completed");
        matchedComplete = true;
      }
      if (isCompletedItem(item)) completed.push({ horizon: h.title, item });
      else active.push(item);
    }
    parsed[h.key] = { rel, prefix: sec.prefix, items: active };
  }
  if (!matchedComplete) throw new Error(`No roadmap item matched --complete '${completeTarget}'`);
  if (completed.length > 0 && checkOnly) invariantErrors.push("docs/roadmap/* contains unrolled Completed items; run npm run docs:roll");

  if (!checkOnly && completed.length > 0) {
    const archiveRel = "docs/roadmap/archive.md";
    const archiveRaw = sfs.read(archiveRel);
    const bullets = completed.map(({ horizon, item }) => {
      const goal = itemField(item, "Goal") || itemField(item, "Question") || "Completed.";
      return `- **[${horizon}] ${cleanTitle(item.title)}** — ${goal}`;
    }).join("\n");
    const idx = archiveRaw.indexOf("\n## ");
    const block = `## Rolled ${today}\n\n${bullets}\n\n`;
    const nextArchive = idx >= 0 ? `${archiveRaw.slice(0, idx + 1)}${block}${archiveRaw.slice(idx + 1)}` : `${archiveRaw.trimEnd()}\n\n${block}`;
    sfs.stage(archiveRel, nextArchive, staleFiles);
  }

  let promotedCount = 0;
  if (!checkOnly && promote) {
    while (parsed.now.items.length < NOW_WIP_TARGET) {
      const rIdx = parsed.next.items.findIndex((it) => itemField(it, "Status").toLowerCase() === "ready");
      if (rIdx < 0) break;
      const [item] = parsed.next.items.splice(rIdx, 1);
      setItemStatus(item, "Active");
      parsed.now.items.push(item);
      promotedCount += 1;
    }
    while (parsed.next.items.length < NEXT_WIP_TARGET && parsed.later.items.length > 0) {
      const lIdx = parsed.later.items.findIndex((it) => ["ready", "planned"].includes(itemField(it, "Status").toLowerCase()));
      if (lIdx < 0) break;
      const [item] = parsed.later.items.splice(lIdx, 1);
      setItemStatus(item, "Ready");
      parsed.next.items.push(item);
      promotedCount += 1;
    }
  }

  for (const h of HORIZONS) {
    parsed[h.key].items = renumberItems(parsed[h.key].items);
    if (!checkOnly) sfs.stage(parsed[h.key].rel, formatH3Items(parsed[h.key].prefix, parsed[h.key].items), staleFiles);
  }

  const decisions = splitSections(sfs.read("docs/roadmap/decision-log.md"), "### ").items;
  const archivedCount = [...sfs.read("docs/roadmap/archive.md").matchAll(/^- /gm)].length;

  const roadmapIndexBlock = [
    "## Horizon Files", "", "| File | Purpose | Active Items |", "|---|---|---:|",
    ...HORIZONS.map((h) => `| [\`${h.file}\`](${h.file}) | ${h.desc} | ${parsed[h.key].items.length} |`),
    `| [\`decision-log.md\`](decision-log.md) | Durable strategic & governance decisions | ${decisions.length} |`,
    `| [\`archive.md\`](archive.md) | Rolled & completed milestones | ${archivedCount} |`,
  ].join("\n");
  sfs.stage("docs/roadmap/README.md", replaceAutorollBlock(sfs.read("docs/roadmap/README.md"), roadmapIndexBlock, "docs/roadmap/README.md"), staleFiles);

  const renderList = (items) => items.map((it) => {
    const st = itemField(it, "Status");
    const summary = rewriteRelativeLinks(itemField(it, "Goal") || itemField(it, "Question"), "docs/roadmap", ".");
    return `- **${cleanTitle(it.title)}**${st ? ` *(${st})*` : ""}: ${summary}`;
  }).join("\n");

  const existingRootRoadmap = sfs.read("ROADMAP.md");
  const existingSyncDate = existingRootRoadmap.match(/Synced (\d{4}-\d{2}-\d{2})/)?.[1];
  const buildRootRoadmapBlock = (syncDate) => [
    `## Horizon Index (\`docs/roadmap/\` · Synced ${syncDate})`, "",
    "| Horizon | Focus | Items | Source |", "|---|---|---:|---|",
    ...HORIZONS.map((h) => `| **${h.title}** | ${h.desc} | ${parsed[h.key].items.length} | [\`docs/roadmap/${h.file}\`](docs/roadmap/${h.file}) |`),
    `| **Decision Log** | Durable architectural & governance decisions | ${decisions.length} | [\`docs/roadmap/decision-log.md\`](docs/roadmap/decision-log.md) |`,
    `| **Completed Archive** | Rolled & completed milestones | ${archivedCount} | [\`docs/roadmap/archive.md\`](docs/roadmap/archive.md) |`, "",
    "## Now", "", renderList(parsed.now.items) || "- *Empty — add or promote items from `docs/roadmap/next.md`.*", "",
    "## Next", "", renderList(parsed.next.items) || "- *Empty — add queued items in `docs/roadmap/next.md`.*", "",
    "## Later & Discovery", "",
    ...parsed.later.items.map((it) => `- **Later — ${cleanTitle(it.title)}:** ${rewriteRelativeLinks(itemField(it, "Goal"), "docs/roadmap", ".")}`),
    ...parsed.discovery.items.map((it) => `- **Discovery — ${cleanTitle(it.title)}:** ${rewriteRelativeLinks(itemField(it, "Question"), "docs/roadmap", ".")}`), "",
    "## Recent Decisions", "",
    ...decisions.slice(0, 3).map((d) => `- **${d.title}** — ${rewriteRelativeLinks(itemField(d, "Decision"), "docs/roadmap", ".")}`),
  ].join("\n");

  // Midnight-safe date stability: if only the date changed and --date was not forced, keep existing date.
  const candidateSameDate = existingSyncDate && !explicitDate
    ? replaceAutorollBlock(existingRootRoadmap, buildRootRoadmapBlock(existingSyncDate), "ROADMAP.md")
    : null;
  const nextRootRoadmap = candidateSameDate === existingRootRoadmap
    ? candidateSameDate
    : replaceAutorollBlock(existingRootRoadmap, buildRootRoadmapBlock(today), "ROADMAP.md");

  if (nextRootRoadmap.split(/\r?\n/).length > MAX_ROOT_LINES) invariantErrors.push(`ROADMAP.md exceeds ${MAX_ROOT_LINES} line cap`);
  sfs.stage("ROADMAP.md", nextRootRoadmap, staleFiles);
  return { rolledRoadmapCount: completed.length, promotedCount };
}

function summarizeSliceText(rel, text) {
  if (!text.startsWith("<!-- [LAYER: UI] -->")) throw new Error(`${rel} missing <!-- [LAYER: UI] --> header`);
  const titleMatch = text.match(/^#\s+(.+)$/m);
  const title = titleMatch ? titleMatch[1].trim() : path.basename(rel);
  const bullets = [...text.matchAll(/^- \*\*([^*]+)\*\*(.*)$/gm)].map((m) => ({
    heading: m[1].trim(),
    rest: m[2].trimEnd(),
  }));
  return { title, bullets, text };
}

function insertChangelogEntry(raw, title, section, summary, refsList) {
  if (raw.includes(`- **${title}**`)) return raw;
  const refs = refsList.map((r) => `[\`${path.basename(r)}\`](${rewriteRelativeLinks(`[x](${r})`, ".", "docs/changelog").slice(4, -1)})`).join(", ");
  const headerPart = refs ? `- **${title}** (${refs}):` : `- **${title}**:`;
  const bulletBlock = summary ? `${headerPart}\n  - ${summary}` : headerPart;
  const heading = `### ${section}`;
  const idx = raw.indexOf(heading);
  if (idx >= 0) {
    const lineEnd = raw.indexOf("\n", idx);
    const insertAt = lineEnd >= 0 ? lineEnd + 1 : raw.length;
    return `${raw.slice(0, insertAt)}\n${bulletBlock}\n${raw.slice(insertAt).replace(/^\n+/, "\n")}`;
  }
  return `${raw.trimEnd()}\n\n${heading}\n\n${bulletBlock}\n`;
}

function rotateOverflowUnreleased(sfs, raw, staleFiles) {
  const lines = raw.split(/\r?\n/);
  const topStarts = [];
  for (let i = 0; i < lines.length; i++) if (/^- \*\*/.test(lines[i])) topStarts.push(i);
  if (topStarts.length <= maxUnreleased) return raw;
  const keepCount = Math.max(3, Math.floor(maxUnreleased / 2));
  const cutLine = topStarts[keepCount];
  const overflowLines = lines.slice(cutLine).join("\n").trim();
  const kept = `${lines.slice(0, cutLine).join("\n").trimEnd()}\n`;
  let seq = 1;
  let archiveName = `${today.slice(0, 7)}-cycle-${seq}.md`;
  while (sfs.exists(`docs/changelog/${archiveName}`)) {
    seq += 1;
    archiveName = `${today.slice(0, 7)}-cycle-${seq}.md`;
  }
  sfs.stage(`docs/changelog/${archiveName}`, `<!-- [LAYER: UI] -->\n# Changelog Archive — ${today.slice(0, 7)} Cycle ${seq}\n\n### Rolled Entries\n\n${overflowLines}\n`, staleFiles);
  return kept;
}

function performReleaseRoll(sfs, version, unreleasedText, staleFiles) {
  if (!/^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$/.test(version)) throw new Error(`Invalid --release version '${version}'`);
  if (!/^- /m.test(unreleasedText)) throw new Error("docs/changelog/unreleased.md has no bullet entries to release");
  const bodyIdx = unreleasedText.indexOf("### ");
  const body = bodyIdx >= 0 ? unreleasedText.slice(bodyIdx).trim() : unreleasedText.trim();
  const targetRel = `docs/changelog/v${version}.md`;
  if (sfs.exists(targetRel)) throw new Error(`${targetRel} already exists`);
  sfs.stage(targetRel, `<!-- [LAYER: UI] -->\n# Changelog — v${version} (${today})\n\n${body}\n`, staleFiles);
  sfs.stage("docs/changelog/unreleased.md", [
    "<!-- [LAYER: UI] -->", "# Changelog — Unreleased", "",
    "Active unreleased changes staged for the next release. Run `node bin/docs-roll.mjs --release <version>` to roll this file into a versioned release slice.", "",
    "### Security & Hardening", "", "### Added", "", "### Changed", "", "### Fixed", "",
  ].join("\n"), staleFiles);
}

function rollChangelog(sfs, staleFiles, invariantErrors) {
  const unreleasedRel = "docs/changelog/unreleased.md";
  let unreleasedText = sfs.read(unreleasedRel);
  if (!checkOnly && fromGitRef) {
    const out = execFileSync("git", ["log", "--pretty=format:%s", `${fromGitRef}..HEAD`], { cwd: root, encoding: "utf8" });
    for (const subj of out.split(/\r?\n/).map((s) => s.trim()).filter(Boolean).slice(0, 10)) {
      const sec = /^sec|^security/i.test(subj) ? "Security & Hardening" : /^fix/i.test(subj) ? "Fixed" : /^feat/i.test(subj) ? "Added" : "Changed";
      unreleasedText = insertChangelogEntry(unreleasedText, subj, sec, "", []);
    }
    sfs.stage(unreleasedRel, unreleasedText, staleFiles);
  }
  if (!checkOnly && addChangelogTitle) {
    unreleasedText = insertChangelogEntry(unreleasedText, addChangelogTitle, addChangelogSection, addChangelogSummary, addChangelogRefs);
    sfs.stage(unreleasedRel, unreleasedText, staleFiles);
  }
  if (!checkOnly && !releaseVersion) {
    const rotated = rotateOverflowUnreleased(sfs, unreleasedText, staleFiles);
    if (rotated !== unreleasedText) {
      unreleasedText = rotated;
      sfs.stage(unreleasedRel, unreleasedText, staleFiles);
    }
  }
  if (!checkOnly && releaseVersion) {
    performReleaseRoll(sfs, releaseVersion, unreleasedText, staleFiles);
    unreleasedText = sfs.read(unreleasedRel);
  }

  const diskFiles = fs.readdirSync(path.join(root, "docs/changelog")).filter((f) => f.endsWith(".md") && f !== "README.md");
  const stagedFiles = [...sfs.staged.keys()]
    .filter((k) => k.startsWith("docs/changelog/") && k.endsWith(".md") && k !== "docs/changelog/README.md")
    .map((k) => path.basename(k));
  const allFiles = [...new Set([...diskFiles, ...stagedFiles])].sort((a, b) => (a === "unreleased.md" ? -1 : b === "unreleased.md" ? 1 : b.localeCompare(a)));

  const rows = [];
  const rootArchiveRows = [];
  let unreleasedSummary = null;
  for (const name of allFiles) {
    const rel = `docs/changelog/${name}`;
    const info = summarizeSliceText(rel, sfs.read(rel));
    if (name === "unreleased.md") unreleasedSummary = info;
    const count = info.bullets.length || [...info.text.matchAll(/^- /gm)].length;
    const scope = info.title.replace(/^Changelog(?:\s+Archive)?\s+—\s+/i, "");
    rows.push(`| [\`${name}\`](${name}) | ${scope} | ${count} |`);
    rootArchiveRows.push(`| [\`docs/changelog/${name}\`](docs/changelog/${name}) | ${scope} | ${count} |`);
  }

  sfs.stage("docs/changelog/README.md", replaceAutorollBlock(sfs.read("docs/changelog/README.md"), ["## Changelog Slices", "", "| Slice | Scope | Entries |", "|---|---|---:|", ...rows].join("\n"), "docs/changelog/README.md"), staleFiles);

  const highlights = unreleasedSummary && unreleasedSummary.bullets.length > 0
    ? unreleasedSummary.bullets.map((b) => `- **${b.heading}**${rewriteRelativeLinks(b.rest, "docs/changelog", ".")}`).join("\n")
    : "- No unreleased changes staged. See recent releases below.";
  const rootClBlock = [
    "## Unreleased Highlights", "", highlights, "",
    "See [`docs/changelog/unreleased.md`](docs/changelog/unreleased.md) for full detailed sub-bullets.", "",
    "## Changelog Archive Index (`docs/changelog/`)", "", "| Slice | Scope | Entries |", "|---|---|---:|", ...rootArchiveRows,
  ].join("\n");
  const nextRootCl = replaceAutorollBlock(sfs.read("CHANGELOG.md"), rootClBlock, "CHANGELOG.md");
  if (nextRootCl.split(/\r?\n/).length > MAX_ROOT_LINES) invariantErrors.push(`CHANGELOG.md exceeds ${MAX_ROOT_LINES} line cap`);
  sfs.stage("CHANGELOG.md", nextRootCl, staleFiles);
  return { slicesCount: allFiles.length, unreleasedCount: unreleasedSummary?.bullets.length ?? 0 };
}

const REQUIRED_ROLL_FILES = [
  "ROADMAP.md", "CHANGELOG.md",
  "docs/roadmap/README.md", "docs/roadmap/now.md", "docs/roadmap/next.md",
  "docs/roadmap/later.md", "docs/roadmap/discovery.md", "docs/roadmap/decision-log.md", "docs/roadmap/archive.md",
  "docs/changelog/README.md", "docs/changelog/unreleased.md",
];
const missingRollFiles = REQUIRED_ROLL_FILES.filter((rel) => !fs.existsSync(path.join(root, rel)));
if (missingRollFiles.length > 0) {
  const errPayload = { ok: false, checkOnly, dryRun, staleFiles: [], invariantErrors: missingRollFiles.map((f) => `Missing required autoroll file: ${f}`) };
  if (jsonOutput) console.log(JSON.stringify(errPayload, null, 2));
  else console.error(`docs:roll failed — missing required files:\n${missingRollFiles.map((f) => `  - ${f}`).join("\n")}`);
  process.exit(1);
}

const sfs = new StagedFs();
const staleFiles = [];
const invariantErrors = [];
const roadmapStats = rollRoadmap(sfs, staleFiles, invariantErrors);
const changelogStats = rollChangelog(sfs, staleFiles, invariantErrors);

if (invariantErrors.length > 0) {
  if (jsonOutput) {
    console.log(JSON.stringify({ ok: false, checkOnly, dryRun, roadmapStats, changelogStats, staleFiles, invariantErrors }, null, 2));
  } else {
    console.error("docs:roll failed — invariant violations detected:");
    for (const e of invariantErrors) console.error(`  - ${e}`);
  }
  process.exit(1);
}

if (!checkOnly && !dryRun && staleFiles.length > 0) {
  sfs.commitAtomic();
}

const ok = !checkOnly || staleFiles.length === 0;
if (jsonOutput) {
  console.log(JSON.stringify({ ok, checkOnly, dryRun, roadmapStats, changelogStats, staleFiles, invariantErrors }, null, 2));
} else if (checkOnly) {
  if (!ok) {
    console.error("docs:roll:check failed — run `npm run docs:roll` to synchronize:");
    for (const f of staleFiles) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log("docs:roll:check OK — ROADMAP.md, CHANGELOG.md, docs/roadmap/, and docs/changelog/ are synchronized");
} else {
  console.log(`docs:roll${dryRun ? " (dry-run)" : ""} OK — ${staleFiles.length} file(s) updated; roadmap rolled=${roadmapStats.rolledRoadmapCount}, promoted=${roadmapStats.promotedCount}; changelog slices=${changelogStats.slicesCount}, unreleased=${changelogStats.unreleasedCount}`);
}
if (!ok) process.exit(1);
