#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * GALXAI Documentation Sovereignty — Zero-Dependency Hardened Docs Checker (v3.0.0 Zenith).
 *
 * Enforces:
 *  1. Required root, docs/, spec/, and workflows/ workspace documents (DOC-REQ-001).
 *  2. Exact [LAYER: <ZONE>] headers on Markdown (.md) and config/script files (.sh, .yml, .yaml, .toml) (DOC-LAY-001..002, DOC-CFG-001..003).
 *  3. Linear O(N) line-accurate Markdown parser: handles nested 4-backtick fences, catches unlabeled code fences (DOC-STY-003), unclosed fences (DOC-STY-004), and missing/duplicate H1 (DOC-STY-001..002).
 *  4. Repository-relative links only for BOTH Markdown `[...](...)` AND HTML `<a href>` / `<img src>`: blocks file://, absolute paths, root-escaping ../, and broken targets (DOC-LNK-001..005).
 *  5. Exact GitHub-slugger #anchor verification for same-file and cross-file headings (DOC-ANC-001).
 *  6. Secret token & local machine-path leak detection with exact line numbers (DOC-SEC-001..005, DOC-LNK-004).
 *  7. Mandatory template headings for ADRs (ADR-*), Playbooks (PB-*), and Post-mortems (PM-*) (DOC-TPL-001).
 *  8. Real parsed Markdown link verification for 100% directory README.md indexing & docs/README.md portal reachability (DOC-IDX-001..002).
 *  9. Autoroll marker balance & <=160-line cap on ROADMAP.md and CHANGELOG.md (DOC-ROL-001..002).
 * 10. README.md GALXAI structural blueprint (DOC-RDM-001) & llms.txt local path integrity (DOC-LLM-001).
 * 11. CI & package.json wiring parity (DOC-CI-001..002).
 *
 * Formats: human-readable (default) | --json | --sarif (SARIF 2.1.0 with exact 1-based line numbers).
 */
import fs from "node:fs";
import path from "node:path";

const argv = process.argv.slice(2);
if (argv.includes("--help") || argv.includes("-h")) {
  console.log(`Usage: node bin/docs-check.mjs [rootDir] [--json | --sarif] [--strict-readme] [--skip-ci]`);
  process.exit(0);
}

const wantJson = argv.includes("--json");
const wantSarif = argv.includes("--sarif");
const strictReadme = argv.includes("--strict-readme");
const skipCi = argv.includes("--skip-ci");
const rootArg = argv.find((a) => !a.startsWith("-")) ?? ".";
const root = path.resolve(rootArg);

const DEFAULT_IGNORE_DIRS = new Set([
  ".git", "target", "vendor", "node_modules", ".joy-state", ".dietcode",
  "build", "dist", ".next", ".turbo", ".cache", "HEAV3NS-MICRO", "HEAV3NS-PRESS",
]);

const DEFAULT_REQUIRED_FILES = [
  "README.md", "AGENTS.md", "SOUL.md", "VALIDATION_REPORT.md",
  "CONTRIBUTING.md", "CODE_OF_CONDUCT.md", "SECURITY.md", "SUPPORT.md",
  "CHANGELOG.md", "ROADMAP.md", "LICENSE",
  "docs/README.md", "docs/DOCS_GUIDE.md", "docs/CODE_TO_DOC_MAP.md",
  "docs/CODEBASE_STANDARDS.md", "docs/MAINTAINER.md", "docs/GETTING_STARTED.md",
  "docs/GLOSSARY.md", "docs/OPERATIONS.md", "docs/SECURITY_BEST_PRACTICES.md",
  "docs/RELEASING.md", "docs/CI_INTEGRATION.md", "docs/CONSTITUTION.md",
  "docs/PROJECT_MAP.md",
  "docs/adr/README.md", "docs/adr/0000-adr-template.md",
  "docs/playbooks/README.md", "docs/wiki/README.md", "docs/knowledgebase/README.md",
  "docs/post-mortems/README.md", "docs/post-mortems/0000-postmortem-template.md",
  "docs/roadmap/README.md", "docs/changelog/README.md",
  "spec/00-CANONICAL-MODEL.md", "spec/01-SYNTAX-AND-HEADERS.md",
  "spec/02-DEPENDENCY-AND-COMPOSITION-RULES.md", "spec/03-METRICS-AND-FORENSICS.md",
  "spec/04-ENFORCEMENT-AND-LENIENCY.md", "spec/05-PORTABLE-SUBSTRATE-TRINITY.md",
  "workflows/01-TRIAD-AUDIT-PLANNING.md", "workflows/02-TACTICAL-REFACTORING-SOPS.md",
  "workflows/03-ADOPTING-IN-EXISTING-PROJECTS.md", "workflows/04-EXECUTION-ERGONOMICS-JOYRIDE.md",
  "workflows/05-PORTABLE-SPIDER-AND-BROCCOLIDB.md",
];

const INDEXED_COLLECTIONS = [
  { dir: "docs", index: "README.md", exclude: new Set() },
  { dir: "docs/adr", index: "README.md", exclude: new Set(["0000-adr-template.md"]) },
  { dir: "docs/playbooks", index: "README.md", exclude: new Set() },
  { dir: "docs/post-mortems", index: "README.md", exclude: new Set(["0000-postmortem-template.md"]) },
  { dir: "docs/knowledgebase", index: "README.md", exclude: new Set() },
  { dir: "docs/wiki", index: "README.md", exclude: new Set() },
  { dir: "docs/roadmap", index: "README.md", exclude: new Set() },
  { dir: "docs/changelog", index: "README.md", exclude: new Set() },
  { dir: "docs/reports", index: "README.md", exclude: new Set() },
  { dir: "docs/strategy", index: "README.md", exclude: new Set() },
  { dir: "docs/commissions", index: "README.md", exclude: new Set() },
  { dir: "docs/i18n", index: "README.md", exclude: new Set() },
];

const SECRET_PATTERNS = [
  { id: "DOC-SEC-001", label: "OpenAI/Stripe secret key", re: /\b(?:sk-proj-|sk-live-|sk_live_|rk_live_)[A-Za-z0-9_-]{16,}\b/g },
  { id: "DOC-SEC-002", label: "GitHub personal access token", re: /\b(?:ghp_|gho_|ghu_|github_pat_)[A-Za-z0-9_]{20,}\b/g },
  { id: "DOC-SEC-003", label: "AWS access key ID", re: /\bAKIA[0-9A-Z]{16}\b/g },
  { id: "DOC-SEC-004", label: "Google API key", re: /\bAIza[0-9A-Za-z_-]{35}\b/g },
  { id: "DOC-SEC-005", label: "Private key PEM block", re: /-----BEGIN (?:RSA |EC |OPENSSH |ED25519 )?PRIVATE KEY-----/g },
];

const MACHINE_PATH_RE = /(?:^|[\s(`"'])(?:\/Users\/[A-Za-z0-9._-]+\/|\/home\/[A-Za-z0-9._-]+\/|[A-Z]:\\Users\\[A-Za-z0-9._-]+\\)/g;

const findings = [];
function addFinding(ruleId, file, message, line = 1, severity = "error") {
  findings.push({ ruleId, file, line, severity, message });
}

function loadConfig() {
  const cfgPath = path.join(root, ".docs-check.json");
  if (!fs.existsSync(cfgPath)) return {};
  try {
    return JSON.parse(fs.readFileSync(cfgPath, "utf8"));
  } catch (err) {
    addFinding("DOC-CFG-001", ".docs-check.json", `Invalid JSON in .docs-check.json: ${err.message}`, 1);
    return {};
  }
}

const config = loadConfig();
const ignoreDirs = new Set([...DEFAULT_IGNORE_DIRS, ...(config.ignoreDirs ?? [])]);
const requiredFiles = config.requiredFiles ?? DEFAULT_REQUIRED_FILES;

function toRel(abs) {
  return path.relative(root, abs).split(path.sep).join("/");
}

function walkFiles(dir, predicate, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) continue;
    if (entry.isDirectory()) {
      if (ignoreDirs.has(entry.name)) continue;
      walkFiles(path.join(dir, entry.name), predicate, out);
    } else if (entry.isFile() && predicate(entry.name)) {
      out.push(path.join(dir, entry.name));
    }
  }
  return out;
}

function expectedMdLayer(rel) {
  if (rel.startsWith("spec/")) return "DOMAIN";
  if (rel.startsWith("workflows/")) return "CORE";
  return "UI";
}

function expectedConfigLayer(rel) {
  if (
    rel === "Cargo.toml" || rel === ".cargo/config.toml" || rel === "pyproject.toml" ||
    rel.startsWith(".github/workflows/") || rel.startsWith("docker/") ||
    rel.startsWith("cloudflare/") || rel.startsWith("infra/")
  ) return "INFRASTRUCTURE";
  if (rel.startsWith(".github/") || rel.startsWith("bin/") || rel.startsWith("scripts/")) return "UI";
  return config.configLayers?.[rel];
}

/** Exact GitHub-slugger algorithm (preserves leading '-' from stripped leading emoji). */
function githubSlug(raw) {
  return raw
    .trim()
    .toLowerCase()
    .replace(/<[^>]+>/g, "")
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .replace(/ /g, "-");
}

function stripInlineCode(line) {
  return line.replace(/`+[^`\n]+`+/g, (m) => " ".repeat(m.length));
}

function parseLinkTarget(raw) {
  const trimmed = raw.trim();
  const wrapped = trimmed.match(/^<([^>]+)>/);
  return (wrapped ? wrapped[1] : trimmed.split(/\s+/, 1)[0]).replace(/\\([()])/g, "$1");
}

/** Linear O(N) line-accurate Markdown scanner handling nested fences, headings, links, and unlabeled fences. */
function scanMarkdown(md) {
  const lines = md.split(/\r?\n/);
  const proseLines = [];
  const headings = [];
  const anchors = new Set();
  const anchorCounts = new Map();
  const links = [];
  const unlabeledFences = [];
  let fenceChar = "";
  let fenceLen = 0;
  let fenceOpenLine = 0;

  for (let i = 0; i < lines.length; i += 1) {
    const lineNo = i + 1;
    const line = lines[i];
    const fenceMatch = line.match(/^\s{0,3}(`{3,}|~{3,})(.*)$/);
    if (fenceMatch) {
      const marker = fenceMatch[1];
      const info = fenceMatch[2].trim();
      if (!fenceChar) {
        fenceChar = marker[0];
        fenceLen = marker.length;
        fenceOpenLine = lineNo;
        if (!info) unlabeledFences.push(lineNo);
        continue;
      } else if (marker[0] === fenceChar && marker.length >= fenceLen && !info) {
        fenceChar = "";
        fenceLen = 0;
        fenceOpenLine = 0;
        continue;
      }
    }
    if (fenceChar) continue;

    const clean = stripInlineCode(line);
    proseLines.push({ lineNo, text: clean });

    const hMatch = clean.match(/^(#{1,6})\s+(.+?)\s*#*\s*$/);
    if (hMatch) {
      const level = hMatch[1].length;
      const rawTitle = hMatch[2];
      headings.push({ level, rawTitle, normalized: rawTitle.replace(/^\d+(?:\.\d+)*[.)]?\s+/, "").trim().toLowerCase(), lineNo });
      const base = githubSlug(rawTitle);
      if (base) {
        const seen = anchorCounts.get(base) ?? 0;
        anchorCounts.set(base, seen + 1);
        anchors.add(seen === 0 ? base : `${base}-${seen}`);
      }
    }

    for (const m of clean.matchAll(/!?\[[^\]]*\]\(([^)\n]+)\)/g)) {
      links.push({ target: parseLinkTarget(m[1]), lineNo });
    }
    for (const m of clean.matchAll(/<(?:a|img)\b[^>]*?\b(?:href|src)=["']([^"']+)["']/gi)) {
      links.push({ target: m[1].trim(), lineNo });
    }
  }

  return {
    lineCount: lines.length,
    proseLines,
    headings,
    anchors,
    links,
    unlabeledFences,
    unclosedFenceLine: fenceChar ? fenceOpenLine : 0,
  };
}

// 1. Required files check
for (const rel of requiredFiles) {
  if (!fs.existsSync(path.join(root, rel))) {
    addFinding("DOC-REQ-001", rel, `Required workspace document is missing: ${rel}`, 1);
  }
}

// 2. Parse all Markdown files once in O(N)
const mdFiles = walkFiles(root, (n) => n.toLowerCase().endsWith(".md")).sort();
const mdRaw = new Map();
const mdParsed = new Map();
const absToParsed = new Map();
for (const abs of mdFiles) {
  const rel = toRel(abs);
  const raw = fs.readFileSync(abs, "utf8");
  const parsed = scanMarkdown(raw);
  mdRaw.set(rel, raw);
  mdParsed.set(rel, parsed);
  absToParsed.set(abs, parsed);
}

let linksChecked = 0;
let anchorsChecked = 0;

for (const abs of mdFiles) {
  const rel = toRel(abs);
  const raw = mdRaw.get(rel);
  const parsed = mdParsed.get(rel);
  const expected = expectedMdLayer(rel);

  const header = raw.slice(0, 10_000).match(/^<!--\s*\[LAYER:\s*(DOMAIN|CORE|INFRASTRUCTURE|PLUMBING|UI)\s*\]\s*-->/);
  if (!header) addFinding("DOC-LAY-001", rel, `Must start with <!-- [LAYER: ${expected}] -->`, 1);
  else if (header[1] !== expected) addFinding("DOC-LAY-002", rel, `Tagged [LAYER: ${header[1]}], expected [LAYER: ${expected}]`, 1);

  const h1s = parsed.headings.filter((h) => h.level === 1);
  if (h1s.length === 0) addFinding("DOC-STY-001", rel, "Missing H1 title (# Title)", 1);
  else if (h1s.length > 1) addFinding("DOC-STY-002", rel, `Contains ${h1s.length} H1 headings (expected exactly 1)`, h1s[1].lineNo);

  for (const lineNo of parsed.unlabeledFences) {
    addFinding("DOC-STY-003", rel, "Fenced code block must specify a language identifier (e.g. ```bash, ```text)", lineNo);
  }
  if (parsed.unclosedFenceLine > 0) {
    addFinding("DOC-STY-004", rel, "Unclosed fenced code block detected", parsed.unclosedFenceLine);
  }

  for (const { lineNo, text } of parsed.proseLines) {
    for (const pat of SECRET_PATTERNS) {
      pat.re.lastIndex = 0;
      if (pat.re.test(text)) addFinding(pat.id, rel, `Potential secret leaked in documentation (${pat.label})`, lineNo);
    }
    MACHINE_PATH_RE.lastIndex = 0;
    const leak = MACHINE_PATH_RE.exec(text);
    if (leak) addFinding("DOC-LNK-004", rel, `Machine-specific local path leaked in prose: ${leak[0].trim()}`, lineNo);
  }

  for (const { target, lineNo } of parsed.links) {
    if (!target || /^(https?:|mailto:|tel:|data:)/i.test(target)) continue;
    if (/^file:/i.test(target) || path.isAbsolute(target)) {
      addFinding("DOC-LNK-001", rel, `Machine-specific or absolute link forbidden: ${target}`, lineNo);
      continue;
    }
    const [rawPath, rawAnchor] = target.split("#", 2);
    const pathname = rawPath.split("?", 1)[0];
    let targetAbs = abs;
    if (pathname) {
      linksChecked += 1;
      let decoded;
      try { decoded = decodeURIComponent(pathname); } catch {
        addFinding("DOC-LNK-002", rel, `Invalid URL-escaped link: ${target}`, lineNo);
        continue;
      }
      targetAbs = path.resolve(path.dirname(abs), decoded);
      if (targetAbs !== root && !targetAbs.startsWith(root + path.sep)) {
        addFinding("DOC-LNK-003", rel, `Relative link escapes repository root: ${target}`, lineNo);
        continue;
      }
      if (!fs.existsSync(targetAbs)) {
        addFinding("DOC-LNK-005", rel, `Broken relative link (missing target): ${target}`, lineNo);
        continue;
      }
    }
    if (rawAnchor && targetAbs.toLowerCase().endsWith(".md")) {
      anchorsChecked += 1;
      let decodedAnchor = rawAnchor;
      try { decodedAnchor = decodeURIComponent(rawAnchor); } catch { /* keep raw */ }
      const normalizedAnchor = decodedAnchor.trim().toLowerCase();
      const targetParsed = absToParsed.get(targetAbs);
      if (targetParsed && normalizedAnchor && !targetParsed.anchors.has(normalizedAnchor)) {
        addFinding("DOC-ANC-001", rel, `Broken Markdown anchor '#${decodedAnchor}' in ${target}`, lineNo);
      }
    }
  }
}

// 3. Tagged config/script files
const configFiles = walkFiles(root, (n) => /\.(?:sh|ya?ml|toml)$/i.test(n)).sort();
for (const abs of configFiles) {
  const rel = toRel(abs);
  const expected = expectedConfigLayer(rel);
  if (!expected) continue;
  const head = fs.readFileSync(abs, "utf8").slice(0, 10_000);
  const match = head.match(/^(?:#![^\n]*\n)?#\s*\[LAYER:\s*(DOMAIN|CORE|INFRASTRUCTURE|PLUMBING|UI)\s*\]/);
  if (!match) addFinding("DOC-CFG-002", rel, `Must start with # [LAYER: ${expected}]${rel.endsWith(".sh") ? " after shebang" : ""}`, 1);
  else if (match[1] !== expected) addFinding("DOC-CFG-003", rel, `Tagged [LAYER: ${match[1]}], expected [LAYER: ${expected}]`, 1);
}

// 4. Template heading enforcement (ADR, Playbooks, Post-mortems)
const checkDirTemplates = (dirRel, pattern, req) => {
  const dirAbs = path.join(root, dirRel);
  if (!fs.existsSync(dirAbs)) return;
  for (const name of fs.readdirSync(dirAbs).filter((n) => pattern.test(n))) {
    const rel = `${dirRel}/${name}`;
    const found = (mdParsed.get(rel)?.headings ?? []).filter((h) => h.level === 2 || h.level === 3).map((h) => h.normalized);
    for (const h of req) {
      if (!found.includes(h.toLowerCase())) addFinding("DOC-TPL-001", rel, `Missing required template section: ## ${h}`, 1);
    }
  }
};
checkDirTemplates("docs/adr", /^ADR-\d+.*\.md$/i, ["Status", "Context", "Decision", "Consequences", "Verification"]);
checkDirTemplates("docs/playbooks", /^PB-\d+.*\.md$/i, ["Triage at a glance", "Remediation steps", "Verification"]);
checkDirTemplates("docs/post-mortems", /^PM-\d+.*\.md$/i, ["Summary", "Root cause", "Action items", "Lessons learned", "Verification"]);

// 5. Real link-based index completeness & docs/README.md reachability
let collectionsChecked = 0;
const portalLinks = new Set((mdParsed.get("docs/README.md")?.links ?? []).map((l) => l.target.split("#", 1)[0].replace(/^\.\//, "")));
for (const col of INDEXED_COLLECTIONS) {
  const dirAbs = path.join(root, col.dir);
  const idxRel = `${col.dir}/${col.index}`;
  const idxParsed = mdParsed.get(idxRel);
  if (!idxParsed) continue;
  collectionsChecked += 1;
  const idxTargets = new Set(idxParsed.links.map((l) => l.target.split("#", 1)[0].replace(/^\.\//, "")));
  for (const name of fs.readdirSync(dirAbs).filter((n) => n.endsWith(".md"))) {
    if (name === col.index || col.exclude.has(name)) continue;
    if (!idxTargets.has(name)) {
      addFinding("DOC-IDX-001", idxRel, `Collection index has no Markdown link to ${name}`, 1);
    }
  }
  if (col.dir !== "docs" && mdParsed.has("docs/README.md")) {
    const subRel = `${col.dir.slice("docs/".length)}/${col.index}`;
    if (!portalLinks.has(subRel)) {
      addFinding("DOC-IDX-002", "docs/README.md", `Documentation portal does not link to collection index ${subRel}`, 1);
    }
  }
}

// 6. Autoroll balance & 160-line cap
for (const rolledFile of ["ROADMAP.md", "CHANGELOG.md"]) {
  const raw = mdRaw.get(rolledFile);
  if (!raw) continue;
  const starts = (raw.match(/<!-- AUTOROLL:START -->/g) || []).length;
  const ends = (raw.match(/<!-- AUTOROLL:END -->/g) || []).length;
  if (starts !== 1 || ends !== 1 || raw.indexOf("<!-- AUTOROLL:START -->") > raw.indexOf("<!-- AUTOROLL:END -->")) {
    addFinding("DOC-ROL-001", rolledFile, `${rolledFile} must contain exactly one balanced <!-- AUTOROLL:START --> ... <!-- AUTOROLL:END --> block`, 1);
  }
  const lineCount = mdParsed.get(rolledFile).lineCount;
  if (lineCount > 160) {
    addFinding("DOC-ROL-002", rolledFile, `${rolledFile} has ${lineCount} lines (hard cap: 160 lines)`, 161);
  }
}

// 7. README.md GALXAI blueprint check
const readmeParsed = mdParsed.get("README.md");
if (readmeParsed && (strictReadme || !config.skipReadmeBlueprint)) {
  const h2s = readmeParsed.headings.filter((h) => h.level === 2).map((h) => h.normalized);
  for (const check of [
    { label: "Quickstart", match: (h) => h.includes("quickstart") || h.includes("quick start") },
    { label: "Architecture", match: (h) => h.includes("architecture") },
    { label: "Implementation Boundary", match: (h) => h.includes("boundary") || h.includes("implementation") },
    { label: "Documentation Portal", match: (h) => h.includes("documentation") },
    { label: "Verification / Standards", match: (h) => h.includes("verification") || h.includes("standards") },
  ]) {
    if (!h2s.some(check.match)) addFinding("DOC-RDM-001", "README.md", `README.md missing required GALXAI blueprint section: ${check.label}`, 1);
  }
}

// 8. llms.txt local path integrity check
const llmsPath = path.join(root, "llms.txt");
if (fs.existsSync(llmsPath)) {
  const llmsLines = fs.readFileSync(llmsPath, "utf8").split(/\r?\n/);
  llmsLines.forEach((line, idx) => {
    const bullet = line.match(/^\s*-\s+(?:\[[^\]]+\]\(([^)]+)\)|([^\s:]+\.(?:md|txt|json|toml|yml|yaml|mjs|rs|ts)))\s*$/);
    if (!bullet) return;
    const ref = (bullet[1] || bullet[2] || "").trim();
    if (!ref || /^https?:\/\//i.test(ref)) return;
    const resolved = path.resolve(root, ref.split("#", 1)[0]);
    if (!fs.existsSync(resolved)) {
      addFinding("DOC-LLM-001", "llms.txt", `llms.txt references missing file: ${ref}`, idx + 1);
    }
  });
}

// 9. CI & package.json wiring
if (!skipCi && !config.skipCiCheck) {
  const pkgPath = path.join(root, "package.json");
  if (fs.existsSync(pkgPath)) {
    const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
    if (!pkg.scripts?.verify?.includes("docs:check")) {
      addFinding("DOC-CI-001", "package.json", "package.json scripts.verify must invoke docs:check", 1);
    }
  }
  const ciPath = path.join(root, ".github/workflows/ci.yml");
  if (fs.existsSync(ciPath)) {
    const ci = fs.readFileSync(ciPath, "utf8");
    if (!ci.includes("npm run verify") && !ci.includes("docs:check")) {
      addFinding("DOC-CI-002", ".github/workflows/ci.yml", ".github/workflows/ci.yml must run npm run verify or docs:check", 1);
    }
  }
}

const errors = findings.filter((f) => f.severity === "error");
const passed = errors.length === 0;

if (wantSarif) {
  console.log(JSON.stringify({
    $schema: "https://json.schemastore.org/sarif-2.1.0.json",
    version: "2.1.0",
    runs: [{
      tool: { driver: { name: "galxai-docs-check", version: "3.0.0", informationUri: "docs/DOCS_GUIDE.md" } },
      results: findings.map((f) => ({
        ruleId: f.ruleId,
        level: f.severity === "error" ? "error" : "warning",
        message: { text: f.message },
        locations: [{ physicalLocation: { artifactLocation: { uri: f.file }, region: { startLine: f.line } } }],
      })),
    }],
  }, null, 2));
} else if (wantJson) {
  console.log(JSON.stringify({
    passed,
    requiredDocumentsChecked: requiredFiles.length,
    markdownFilesScanned: mdFiles.length,
    taggedConfigFilesScanned: configFiles.length,
    collectionsChecked,
    linksChecked,
    anchorsChecked,
    errorCount: errors.length,
    findings,
  }, null, 2));
} else if (passed) {
  console.log(
    `docs:check OK — ${requiredFiles.length} required docs, ${mdFiles.length} Markdown files, ${collectionsChecked} indexed collections, ${linksChecked} relative links, ${anchorsChecked} anchors verified`,
  );
} else {
  console.error(`GALXAI documentation check FAILED (${errors.length} error(s)):`);
  for (const f of errors) console.error(`  [${f.ruleId}] ${f.file}:${f.line} — ${f.message}`);
}

process.exit(passed ? 0 : 1);
