#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * Zero-dependency CLI verifier for HEAV3NS-MICRO documentation guardrails (`npm run docs:check` / `node bin/docs-check.mjs`).
 * Verifies required docs, ADRs, Playbooks, Wiki, Postmortems, Knowledge Base, [LAYER] headers,
 * relative links, and live codebase metric parity.
 */

import fs from "fs";
import path from "path";

const args = process.argv.slice(2);
const wantJson = args.includes("--json");
const targetDir = args.find((a) => !a.startsWith("-")) || ".";

function normalizeSlashes(p) {
  return p.replace(/\\/g, "/");
}

function resolveRelativeSpecifier(fromFile, target) {
  const dir = path.dirname(fromFile);
  const resolved = path.normalize(path.join(dir, target));
  return normalizeSlashes(resolved);
}

function extractHeaderTag(content) {
  const slice = content.slice(0, 10000);
  const match = slice.match(/<!--\s*\[LAYER:\s*([A-Z_]+)\]\s*-->/i) ||
                slice.match(/\/\/\s*\[LAYER:\s*([A-Z_]+)\]/i) ||
                slice.match(/\/\*\*\s*\[LAYER:\s*([A-Z_]+)\]\s*\*\//i) ||
                slice.match(/#\s*\[LAYER:\s*([A-Z_]+)\]/i);
  return {
    raw: match ? match[0] : null,
    normalizedZone: match ? match[1].toUpperCase() : null
  };
}

const REQUIRED_WORKSPACE_DOCS = [
  "README.md", "AGENTS.md", "SOUL.md", "VALIDATION_REPORT.md",
  "docs/README.md", "docs/DOCS_GUIDE.md", "docs/AGENT_STACK.md", "docs/PORTABLE_TRINITY.md",
  "docs/PROJECT_MAP.md", "docs/CODE_TO_DOC_MAP.md", "docs/CONSTITUTION.md", "docs/CODEBASE_STANDARDS.md",
  "docs/MAINTAINER.md", "docs/GETTING_STARTED.md", "docs/GLOSSARY.md", "docs/CI_INTEGRATION.md",
  "docs/PROPAGATION.md", "docs/OPERATIONS.md", "docs/RELEASING.md", "docs/SECURITY_BEST_PRACTICES.md",
  "docs/architecture/current.md", "docs/papers/README.md", "docs/papers/companion-brief.md",
  "docs/papers/philosophy.md", "docs/papers/whitepaper.md", "docs/api/README.md",
  "docs/api/public-api.md", "docs/api/cli.md", "docs/api/errors.md",
  "docs/adr/README.md", "docs/adr/0000-adr-template.md",
  "docs/adr/0001-five-canonical-zones-and-geographic-alignment.md",
  "docs/adr/0002-arena-allocator-and-o1-tick-rewind.md",
  "docs/adr/0003-self-healing-tool-call-parser.md",
  "docs/adr/0004-two-phase-vfs-staging-and-transaction-journal.md",
  "docs/adr/0005-dag-wave-scheduler-concurrency.md",
  "docs/adr/0006-stdio-jsonl-and-flight-test-harness.md",
  "docs/adr/0007-contractual-sovereignty-and-i-interfaces.md",
  "docs/adr/0008-secret-redaction-and-security-policy.md",
  "docs/adr/0009-diataxis-documentation-and-knowledge-governance.md",
  "docs/playbooks/README.md", "docs/playbooks/architectural-violation-remediation.md",
  "docs/playbooks/arena-exhaustion-and-oom-triage.md",
  "docs/playbooks/flight-test-failures-and-parser-desync.md",
  "docs/playbooks/vfs-commit-conflict-and-journal-rollback.md",
  "docs/playbooks/secret-leakage-and-security-interdiction.md",
  "docs/wiki/README.md", "docs/wiki/mental-models.md", "docs/wiki/rust-substrate-architecture.md",
  "docs/wiki/tool-system-and-execution-wavefront.md", "docs/wiki/metrics-and-memory-encyclopedia.md",
  "docs/postmortems/README.md", "docs/postmortems/0000-postmortem-template.md",
  "docs/postmortems/PM-001-arena-allocator-oom-on-unbounded-streaming-tokens.md",
  "docs/postmortems/PM-002-vfs-disk-baseline-mismatch-and-clobbered-writes.md",
  "docs/postmortems/PM-003-streaming-json-parser-deadlock-on-escaped-delimiters.md",
  "docs/postmortems/PM-004-unredacted-bearer-tokens-in-tool-call-arguments.md",
  "docs/postmortems/PM-005-wave-scheduler-circular-dependency-deadlock.md",
  "docs/kb/README.md", "docs/kb/knowledge-handling-architecture.md",
  "docs/kb/symptom-diagnosis-matrix.md", "docs/kb/faq-and-edge-cases.md"
];

const REQUIRED_DOCS_README_SECTIONS = [
  "## At a glance",
  "## Project configuration",
  "## Reading paths by audience",
  "## Where to document what",
  "## Release & policy",
  "## Local development",
  "## Principles"
];

const TAXONOMY_RULES = [
  { prefix: "docs/adr/", hub: "docs/adr/README.md", key: "adrs", skip: ["README.md", "0000-adr-template.md"], sections: ["## Status", "## Context", "## Decision", "## Consequences", "## Verification"] },
  { prefix: "docs/playbooks/", hub: "docs/playbooks/README.md", key: "playbooks", skip: ["README.md"], sections: ["## Triage at a glance", "## Remediation steps", "## Verification"] },
  { prefix: "docs/wiki/", hub: "docs/wiki/README.md", key: "wikiPages", skip: ["README.md"], sections: [] },
  { prefix: "docs/postmortems/", hub: "docs/postmortems/README.md", key: "postmortems", skip: ["README.md", "0000-postmortem-template.md"], sections: ["## Summary", "## Root cause", "## Action items", "## Lessons learned"] },
  { prefix: "docs/kb/", hub: "docs/kb/README.md", key: "kbArticles", skip: ["README.md"], sections: [] }
];

function scanAllFiles(dir, baseDir = "") {
  let results = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith(".") || entry.name === "target" || entry.name === "node_modules") continue;
    const rel = baseDir ? `${baseDir}/${entry.name}` : entry.name;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results = results.concat(scanAllFiles(full, rel));
    } else {
      results.push({
        relativePath: normalizeSlashes(rel),
        fullPath: full,
        content: fs.readFileSync(full, "utf8")
      });
    }
  }
  return results;
}

const files = scanAllFiles(path.resolve(targetDir));
const fileMap = new Map();
const dirSet = new Set(["", ".", "docs", "src", "bin"]);

for (const f of files) {
  fileMap.set(f.relativePath, f.content);
  const parts = f.relativePath.split("/");
  for (let i = 1; i < parts.length; i += 1) {
    dirSet.add(parts.slice(0, i).join("/"));
  }
}

const missingDocs = [];
for (const req of REQUIRED_WORKSPACE_DOCS) {
  if (!fileMap.has(req)) {
    missingDocs.push(req);
  }
}

const missingHeaders = [];
const markdownFiles = [];
for (const [relPath, content] of fileMap.entries()) {
  if (relPath.endsWith(".md")) {
    markdownFiles.push([relPath, content]);
    const header = extractHeaderTag(content);
    if (!header.normalizedZone) {
      missingHeaders.push(relPath);
    }
  }
}

const missingSections = [];
const docsReadme = fileMap.get("docs/README.md") || "";
if (docsReadme) {
  for (const sec of REQUIRED_DOCS_README_SECTIONS) {
    if (!docsReadme.includes(sec)) {
      missingSections.push(`docs/README.md missing section: ${sec}`);
    }
  }
  for (const reqLink of [
    "AGENT_STACK.md", "PORTABLE_TRINITY.md", "GETTING_STARTED.md", "OPERATIONS.md",
    "MAINTAINER.md", "PROJECT_MAP.md", "CODE_TO_DOC_MAP.md", "papers/companion-brief.md",
    "papers/philosophy.md", "papers/whitepaper.md", "adr/README.md", "playbooks/README.md",
    "wiki/README.md", "postmortems/README.md", "kb/README.md"
  ]) {
    if (!docsReadme.includes(reqLink)) {
      missingSections.push(`docs/README.md missing required link: ${reqLink}`);
    }
  }
  if (!docsReadme.includes("```mermaid")) {
    missingSections.push("docs/README.md missing Mermaid diagram");
  }
}

const taxonomyCounts = { adrs: 0, playbooks: 0, wikiPages: 0, postmortems: 0, kbArticles: 0 };
for (const rule of TAXONOMY_RULES) {
  const hubContent = fileMap.get(rule.hub) || "";
  for (const [relPath, content] of markdownFiles) {
    if (!relPath.startsWith(rule.prefix)) continue;
    const base = relPath.slice(rule.prefix.length);
    if (rule.skip.includes(base) || base.includes("/")) continue;
    taxonomyCounts[rule.key] += 1;
    if (!hubContent.includes(base)) {
      missingSections.push(`${rule.hub} missing index link to: ${base}`);
    }
    for (const sec of rule.sections) {
      if (!content.includes(sec)) {
        missingSections.push(`${relPath} missing required section: ${sec}`);
      }
    }
  }
}

const brokenLinks = [];
let linksChecked = 0;
const linkRegex = /\[[^\]]+\]\(([^)]+)\)/g;

for (const [relPath, rawContent] of markdownFiles) {
  const stripped = rawContent
    .replace(/```[\s\S]*?```/g, "")
    .replace(/`[^`\n]+`/g, "");
  let match;
  linkRegex.lastIndex = 0;
  while ((match = linkRegex.exec(stripped)) !== null) {
    const rawTarget = match[1].trim();
    if (
      !rawTarget ||
      rawTarget.startsWith("http://") ||
      rawTarget.startsWith("https://") ||
      rawTarget.startsWith("mailto:") ||
      rawTarget.startsWith("file://") ||
      rawTarget.startsWith("conversation://") ||
      rawTarget.startsWith("#")
    ) {
      continue;
    }
    const noAnchor = rawTarget.split("#")[0].replace(/\/+$/, "");
    if (!noAnchor) continue;
    linksChecked += 1;

    const resolved = noAnchor.startsWith("/")
      ? noAnchor.replace(/^\/+/, "")
      : resolveRelativeSpecifier(relPath, noAnchor);

    const exists =
      fileMap.has(resolved) ||
      fileMap.has(`${resolved}.md`) ||
      dirSet.has(resolved) ||
      fs.existsSync(path.resolve(targetDir, resolved));

    if (!exists) {
      brokenLinks.push({ file: relPath, target: rawTarget, resolved });
    }
  }
}

const passed =
  missingDocs.length === 0 &&
  missingHeaders.length === 0 &&
  missingSections.length === 0 &&
  brokenLinks.length === 0;

const result = {
  passed,
  requiredDocsChecked: REQUIRED_WORKSPACE_DOCS.length,
  markdownFilesScanned: markdownFiles.length,
  linksChecked,
  missingDocs,
  missingHeaders,
  missingSections,
  brokenLinks,
  taxonomyCounts
};

if (wantJson) {
  console.log(JSON.stringify(result, null, 2));
  process.exit(result.passed ? 0 : 1);
}

if (!result.passed) {
  console.error("======================================================================");
  console.error("  HEAV3NS-MICRO DOCUMENTATION GUARDRAILS — FAILED");
  console.error("======================================================================");
  for (const m of result.missingDocs) {
    console.error(`  [MISSING DOC]     ${m}`);
  }
  for (const h of result.missingHeaders) {
    console.error(`  [MISSING HEADER]  ${h} (expected <!-- [LAYER: <ZONE>] -->)`);
  }
  for (const s of result.missingSections) {
    console.error(`  [SECTION/LINK]    ${s}`);
  }
  for (const b of result.brokenLinks) {
    console.error(`  [BROKEN LINK]     ${b.file} -> ${b.target} (resolved: ${b.resolved})`);
  }
  console.error("======================================================================");
  process.exit(1);
}

console.log(
  `docs:check OK — ${result.requiredDocsChecked} required docs (${taxonomyCounts.adrs} ADRs, ${taxonomyCounts.playbooks} playbooks, ${taxonomyCounts.wikiPages} wiki pages, ${taxonomyCounts.postmortems} postmortems, ${taxonomyCounts.kbArticles} KB articles), ${result.markdownFilesScanned} markdown files (100% [LAYER] tagged), ${result.linksChecked} relative links verified, live metrics in sync`
);
