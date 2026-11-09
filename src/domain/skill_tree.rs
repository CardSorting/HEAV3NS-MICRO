// [LAYER: DOMAIN]
use crate::plumbing::hash_and_math::fnv1a_32;
use crate::plumbing::secret_redactor::SafetyPolicy;
use crate::plumbing::vision_and_dom::sanitize_trojan_unicode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tier: u8,
    pub prerequisites: Vec<String>,
    pub synergies: Vec<String>,
    pub tags: Vec<String>,
    pub mastery: u8,
    pub unlocked: bool,
    pub instructions: String,
    pub source_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulProfile {
    pub id: String,
    pub name: String,
    pub archetype: String,
    pub alignment: String,
    pub axioms: Vec<String>,
    pub traits: BTreeMap<String, f64>,
    pub directives: Vec<String>,
    pub integrity_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTreeCatalog {
    pub nodes: Vec<SkillNode>,
    pub active_soul: SoulProfile,
    pub competency_scores: BTreeMap<String, f64>,
}

impl Default for SkillTreeCatalog {
    fn default() -> Self { Self::new() }
}

fn seed_node(id: &str, name: &str, desc: &str, cat: &str, tier: u8, prereqs: &[&str], tags: &[&str], instr: &str) -> SkillNode {
    SkillNode {
        id: id.into(), name: name.into(), description: desc.into(), category: cat.into(), tier,
        prerequisites: prereqs.iter().map(|s| (*s).into()).collect(), synergies: Vec::new(),
        tags: tags.iter().map(|s| (*s).into()).collect(), mastery: if tier == 1 { 100 } else { 85 },
        unlocked: true, instructions: instr.into(), source_format: "markdown".into(),
    }
}

impl SkillTreeCatalog {
    pub fn new() -> Self {
        let nodes = vec![
            seed_node("joyzoning-sovereignty", "Joyzoning Architectural Sovereignty", "Enforces 5-zone dependency boundaries, single-line imports, and I<Name> contractual sovereignty.", "architecture", 1, &[], &["architecture", "joyzoning", "sovereignty"], "1. Tag every file with [LAYER: <ZONE>].\n2. Verify pre-edit and post-edit with joy-check.\n3. Keep files under 800 lines."),
            seed_node("computer-use-operator", "Set-of-Marks GUI & Computer Operator", "Operates virtual and native displays via Set-of-Marks (SoM) bounding boxes, undo/redo checkpoints, and action DSL.", "computer-use", 2, &["joyzoning-sovereignty"], &["computer-use", "gui", "som", "display"], "1. Capture frame before mutating UI state.\n2. Target elements by somId or element_id.\n3. Verify post-action state and use undo on unexpected transitions."),
            seed_node("browser-dom-navigator", "Zero-Trust Browser & DOM Navigator", "Executes SSRF-guarded navigation and extracts compressed Set-of-Marks interactive DOM snapshots.", "computer-use", 2, &["computer-use-operator"], &["browser", "cdp", "dom", "ssrf"], "1. Validate target URL against SSRF/IMDS guards.\n2. Snapshot interactive DOM elements before clicking or typing.\n3. Redact sensitive query tokens in telemetry."),
            seed_node("adversarial-auditor", "Adversarial Security & Failure-Mode Auditor", "Hunts edge cases, injection vectors, race conditions, and silent failure paths with concrete root-cause fixes.", "security", 3, &["joyzoning-sovereignty"], &["security", "audit", "hardening", "red-team"], "1. Enumerate trust boundaries and untrusted inputs.\n2. Test boundary conditions, malformed payloads, and path traversal.\n3. Replace monkey patches with structural invariants."),
            seed_node("performance-auditor", "Sub-Millisecond Performance & Token Optimizer", "Profiles algorithmic complexity, memory allocations, and token budget efficiency across hot paths.", "systems", 3, &["joyzoning-sovereignty"], &["performance", "latency", "tokens", "optimization"], "1. Eliminate O(N^2) loops and redundant clones.\n2. Bound output payloads with deterministic token truncation.\n3. Verify p50/p95 latency via micro-benchmarks."),
        ];
        let mut cat = Self { nodes, active_soul: Self::default_soul(), competency_scores: BTreeMap::new() };
        cat.recompute_competencies();
        cat
    }

    pub fn default_soul() -> SoulProfile {
        let mut traits = BTreeMap::new();
        traits.insert("rigor".into(), 0.98);
        traits.insert("conciseness".into(), 0.95);
        traits.insert("autonomy".into(), 0.96);
        traits.insert("creativity".into(), 0.88);
        let axioms = vec![
            "Fix root causes rather than symptoms; never weaken architectural checks.".into(),
            "Verify every mutation with deterministic tests and structural audits.".into(),
            "Preserve user sovereignty, credential confidentiality, and zero-trust boundaries.".into(),
        ];
        let hash = format!("{:08x}", fnv1a_32(axioms.join("|").as_bytes()));
        SoulProfile {
            id: "sovereign-architect".into(),
            name: "Sovereign Systems Architect".into(),
            archetype: "Sovereign Architect".into(),
            alignment: "Lawful Sovereign".into(),
            axioms,
            traits,
            directives: vec![
                "Execute producer-to-consumer (PLUMBING -> DOMAIN -> INFRASTRUCTURE -> CORE -> UI).".into(),
                "Prefer atomic VFS tools over raw shell file redirections.".into(),
            ],
            integrity_hash: hash,
        }
    }

    pub fn recompute_competencies(&mut self) {
        let total = self.nodes.len().max(1) as f64;
        let avg_mastery = self.nodes.iter().map(|n| n.mastery as f64).sum::<f64>() / total;
        self.competency_scores.insert("architecturalSovereignty".into(), (avg_mastery / 100.0).clamp(0.0, 1.0));
        self.competency_scores.insert("computerUseAutonomy".into(), 0.96);
        self.competency_scores.insert("adversarialResilience".into(), 0.97);
        self.competency_scores.insert("tokenEfficiency".into(), 0.95);
    }

    /// Runs Kahn's algorithm for deterministic topological ordering and cycle detection across the skill DAG.
    pub fn evaluate_dag(&self) -> Value {
        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
        let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let known_ids: BTreeSet<String> = self.nodes.iter().map(|n| n.id.clone()).collect();

        for node in &self.nodes {
            in_degree.entry(node.id.clone()).or_insert(0);
            for pre in &node.prerequisites {
                if known_ids.contains(pre) {
                    adj.entry(pre.clone()).or_default().push(node.id.clone());
                    *in_degree.entry(node.id.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree.iter().filter(|(_, &deg)| deg == 0).map(|(k, _)| k.clone()).collect();
        let mut topo_order = Vec::new();
        while let Some(curr) = queue.pop_front() {
            topo_order.push(curr.clone());
            if let Some(children) = adj.get(&curr) {
                for child in children {
                    if let Some(deg) = in_degree.get_mut(child) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        let has_cycle = topo_order.len() != self.nodes.len();
        json!({
            "nodeCount": self.nodes.len(),
            "acyclic": !has_cycle,
            "topologicalOrder": topo_order,
            "competencies": self.competency_scores,
            "nodes": self.nodes
        })
    }

    /// Forges a new skill from a natural-language prompt or 5-step wizard specification.
    pub fn forge_skill(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = sanitize_trojan_unicode(
            args.get("prompt")
                .or_else(|| args.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("custom-sovereign-skill"),
        );
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .map(slugify)
            .unwrap_or_else(|| slugify(&prompt));
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or(&prompt).trim().to_string();
        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("custom").to_string();
        let tier = args.get("tier").and_then(|v| v.as_u64()).unwrap_or(2).clamp(1, 5) as u8;
        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .map(sanitize_trojan_unicode)
            .unwrap_or_else(|| format!("Synthesized capability for: {}", name));
        let instructions = args
            .get("instructions")
            .and_then(|v| v.as_str())
            .map(sanitize_trojan_unicode)
            .unwrap_or_else(|| {
                format!(
                    "1. Analyze target context for {}.\n2. Execute minimal, deterministic mutations.\n3. Verify outcome with tests and structural checks.",
                    name
                )
            });
        let prereqs: Vec<String> = args
            .get("prerequisites")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(|| vec!["joyzoning-sovereignty".to_string()]);
        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(|| vec![category.clone(), "forged".to_string()]);

        let node = SkillNode {
            id: id.clone(),
            name,
            description,
            category,
            tier,
            prerequisites: prereqs,
            synergies: Vec::new(),
            tags,
            mastery: 90,
            unlocked: true,
            instructions,
            source_format: "forged".into(),
        };
        self.upsert_node(node.clone());
        Ok(json!({ "status": "forged", "skill": node, "markdown": Self::render_skill_markdown(&node) }))
    }

    pub fn apply_power_up_pack(&mut self, node_id: &str, pack: &str) -> Result<Value, String> {
        let addon = match pack.trim().to_ascii_lowercase().as_str() {
            "tdd-enforcer" | "tdd" => "\n- [Power-Up: TDD Enforcer] Write failing deterministic tests before mutating implementation code.",
            "security-hardener" | "security" => "\n- [Power-Up: Security Hardener] Validate all inputs, block SSRF/path traversal, and redact secrets.",
            "latency-optimizer" | "performance" => "\n- [Power-Up: Latency Optimizer] Bound allocations, avoid O(N^2) scans, and enforce p95 latency budgets.",
            "accessibility-som" | "som" => "\n- [Power-Up: Set-of-Marks UX] Ensure every interactive widget exposes deterministic role, label, and somId.",
            "joyzoning-strict" | "joyzoning" => "\n- [Power-Up: Joyzoning Strict] Enforce [LAYER: <ZONE>] headers, single-line imports, and <800 line caps.",
            "telemetry-auditor" | "telemetry" => "\n- [Power-Up: Telemetry Auditor] Emit structured progress events and verify zero silent failures.",
            other => return Err(format!("Unknown power-up pack '{other}'. Supported: tdd-enforcer, security-hardener, latency-optimizer, accessibility-som, joyzoning-strict, telemetry-auditor")),
        };
        let node = self.nodes.iter_mut().find(|n| n.id == node_id).ok_or_else(|| format!("Skill '{node_id}' not found"))?;
        if !node.instructions.contains(addon) {
            node.instructions.push_str(addon);
        }
        node.mastery = (node.mastery + 5).min(100);
        let cloned = node.clone();
        self.recompute_competencies();
        Ok(json!({ "status": "power_up_applied", "pack": pack, "skill": cloned }))
    }

    pub fn clone_skill(&mut self, source_id: &str, new_id: &str, new_name: Option<&str>) -> Result<Value, String> {
        let src = self.nodes.iter().find(|n| n.id == source_id).cloned().ok_or_else(|| format!("Source skill '{source_id}' not found"))?;
        let forked = SkillNode {
            id: slugify(new_id),
            name: new_name.map(|s| s.to_string()).unwrap_or_else(|| format!("{} (Fork)", src.name)),
            prerequisites: vec![src.id.clone()],
            ..src
        };
        self.upsert_node(forked.clone());
        Ok(json!({ "status": "cloned", "sourceId": source_id, "skill": forked }))
    }

    pub fn lint_and_autofix_skill(&mut self, node_id: &str, autofix: bool) -> Result<Value, String> {
        let node = self.nodes.iter_mut().find(|n| n.id == node_id).ok_or_else(|| format!("Skill '{node_id}' not found"))?;
        let mut issues = Vec::new();
        if node.description.trim().len() < 16 {
            issues.push("Description is too short (< 16 chars)".to_string());
            if autofix {
                node.description = format!("Sovereign operational capability for {} with deterministic verification.", node.name);
            }
        }
        if node.instructions.trim().len() < 24 {
            issues.push("Instructions lack actionable steps (< 24 chars)".to_string());
            if autofix {
                node.instructions = format!("1. Inspect target scope for {}.\n2. Apply minimal changes.\n3. Verify with tests.", node.name);
            }
        }
        if node.tags.is_empty() {
            issues.push("Skill has no discovery tags".to_string());
            if autofix {
                node.tags.push(node.category.clone());
            }
        }
        let clean = sanitize_trojan_unicode(&node.instructions);
        if clean.len() != node.instructions.len() {
            issues.push("Instructions contained hidden Trojan Unicode codepoints".to_string());
            if autofix {
                node.instructions = clean;
            }
        }
        let threats = SafetyPolicy::scan_for_threats(&node.instructions, "context");
        for t in threats {
            issues.push(format!("Security threat detected in skill instructions: {t}"));
        }
        Ok(json!({
            "skillId": node.id,
            "autofixed": autofix && !issues.is_empty(),
            "issueCount": issues.len(),
            "issues": issues,
            "skill": node
        }))
    }

    pub fn ingest_multi_format(&mut self, filename: &str, raw_content: &str) -> Result<Value, String> {
        let sanitized = sanitize_trojan_unicode(raw_content);
        let lower_name = filename.to_ascii_lowercase();
        if lower_name.ends_with(".soul.md") || lower_name.contains("soul") && lower_name.ends_with(".json") {
            let soul = Self::parse_soul_document(filename, &sanitized);
            self.active_soul = soul.clone();
            return Ok(json!({ "kind": "soul", "profile": soul }));
        }
        let node = Self::parse_skill_document(filename, &sanitized);
        self.upsert_node(node.clone());
        Ok(json!({ "kind": "skill", "skill": node }))
    }

    pub fn parse_skill_document(filename: &str, content: &str) -> SkillNode {
        let trimmed = content.trim();
        // Auto-sense JSON (CharacterCard V2 / OpenAI GPT / Tool JSON)
        if trimmed.starts_with('{') {
            if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                let data = val.get("data").unwrap_or(&val);
                let name = data.get("name").or_else(|| data.get("title")).and_then(|v| v.as_str()).unwrap_or(filename).to_string();
                let desc = data.get("description").or_else(|| data.get("personality")).and_then(|v| v.as_str()).unwrap_or("Imported JSON capability").to_string();
                let instr = data.get("instructions").or_else(|| data.get("system_prompt")).or_else(|| data.get("mes_example")).and_then(|v| v.as_str()).unwrap_or(trimmed).to_string();
                return SkillNode {
                    id: slugify(&name),
                    name,
                    description: desc,
                    category: "imported-json".into(),
                    tier: 2,
                    prerequisites: Vec::new(),
                    synergies: Vec::new(),
                    tags: vec!["imported".into(), "json".into()],
                    mastery: 85,
                    unlocked: true,
                    instructions: instr,
                    source_format: "json".into(),
                };
            }
        }
        // Auto-sense Anthropic Claude XML (<skill> or <instructions>)
        if trimmed.starts_with('<') && (trimmed.contains("<instructions>") || trimmed.contains("<skill")) {
            let name = extract_xml_tag(trimmed, "name").unwrap_or_else(|| filename.to_string());
            let desc = extract_xml_tag(trimmed, "description").unwrap_or_else(|| "Imported Claude XML skill".to_string());
            let instr = extract_xml_tag(trimmed, "instructions").unwrap_or_else(|| trimmed.to_string());
            return SkillNode {
                id: slugify(&name),
                name,
                description: desc,
                category: "imported-xml".into(),
                tier: 2,
                prerequisites: Vec::new(),
                synergies: Vec::new(),
                tags: vec!["imported".into(), "claude-xml".into()],
                mastery: 85,
                unlocked: true,
                instructions: instr,
                source_format: "claude-xml".into(),
            };
        }
        // Default: Markdown with optional YAML frontmatter
        let mut name = filename.trim_end_matches(".md").trim_end_matches(".skill").to_string();
        let mut desc = "Markdown skill capability".to_string();
        let mut category = "general".to_string();
        let mut prereqs = Vec::new();
        for line in trimmed.lines().take(25) {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("name:") {
                name = v.trim().trim_matches('"').trim_matches('\'').to_string();
            } else if let Some(v) = l.strip_prefix("description:") {
                desc = v.trim().trim_matches('"').trim_matches('\'').to_string();
            } else if let Some(v) = l.strip_prefix("category:") {
                category = v.trim().trim_matches('"').trim_matches('\'').to_string();
            } else if let Some(v) = l.strip_prefix("prerequisites:") {
                prereqs = v
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
        SkillNode {
            id: slugify(&name),
            name,
            description: desc,
            category,
            tier: 2,
            prerequisites: prereqs,
            synergies: Vec::new(),
            tags: vec!["markdown".into()],
            mastery: 90,
            unlocked: true,
            instructions: trimmed.to_string(),
            source_format: "markdown".into(),
        }
    }

    pub fn parse_soul_document(filename: &str, content: &str) -> SoulProfile {
        let mut soul = Self::default_soul();
        let clean = sanitize_trojan_unicode(content);
        soul.id = slugify(filename.trim_end_matches(".soul.md").trim_end_matches(".md"));
        for line in clean.lines() {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("name:") {
                soul.name = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = l.strip_prefix("archetype:") {
                soul.archetype = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = l.strip_prefix("- axiom:") {
                soul.axioms.push(v.trim().to_string());
            }
        }
        soul.integrity_hash = format!("{:08x}", fnv1a_32(clean.as_bytes()));
        soul
    }

    pub fn render_skill_markdown(node: &SkillNode) -> String {
        format!(
            "<!-- [LAYER: UI] -->\n---\nname: \"{}\"\ndescription: \"{}\"\ncategory: \"{}\"\ntier: {}\nprerequisites: {:?}\ntags: {:?}\n---\n\n# {}\n\n{}\n",
            node.name, node.description, node.category, node.tier, node.prerequisites, node.tags, node.name, node.instructions
        )
    }

    pub fn export_node(&self, node_id: &str, format: &str) -> Result<Value, String> {
        let node = self.nodes.iter().find(|n| n.id == node_id).ok_or_else(|| format!("Skill '{node_id}' not found"))?;
        let fmt = format.trim().to_ascii_lowercase();
        let payload = match fmt.as_str() {
            "character_card_v2" | "card" => json!({
                "spec": "chara_card_v2",
                "spec_version": "2.0",
                "data": { "name": node.name, "description": node.description, "system_prompt": node.instructions, "tags": node.tags }
            }).to_string(),
            "openai_gpt" | "openai" => json!({
                "name": node.name, "description": node.description, "instructions": node.instructions, "category": node.category
            }).to_string(),
            "claude_xml" | "xml" => format!(
                "<skill id=\"{}\">\n  <name>{}</name>\n  <description>{}</description>\n  <instructions>\n{}\n  </instructions>\n</skill>",
                node.id, node.name, node.description, node.instructions
            ),
            _ => Self::render_skill_markdown(node),
        };
        Ok(json!({ "skillId": node.id, "format": fmt, "content": payload }))
    }

    pub fn upsert_node(&mut self, node: SkillNode) {
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.id == node.id) {
            *existing = node;
        } else {
            self.nodes.push(node);
        }
        self.recompute_competencies();
    }
}

fn slugify(input: &str) -> String {
    let s: String = input
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let collapsed = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if collapsed.is_empty() { "custom-skill".to_string() } else { collapsed }
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}
