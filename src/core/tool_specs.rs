// [LAYER: CORE]
use crate::domain::types::tool_types::{ParamSchema, ParamType, ToolSpec};

/// Returns the 23 built-in tool specifications registered by [`ToolRegistry`](crate::core::tool_registry::ToolRegistry).
pub fn builtin_tool_specs() -> Vec<ToolSpec> {
    use ParamType::*;
    vec![
        ToolSpec {
            name: "view_file",
            description: "Read contents of a file with optional line slicing and byte offset",
            is_mutating: false,
            is_core: true,
            parameters: vec![
                ("path", ParamSchema::req(String, "Absolute or relative file path")),
                ("startLine", ParamSchema::opt(Integer, "1-indexed starting line")),
                ("endLine", ParamSchema::opt(Integer, "1-indexed ending line")),
                ("contentOffset", ParamSchema::opt(Integer, "Byte offset to slice from")),
            ],
        },
        ToolSpec {
            name: "write_file",
            description: "Write content to a file, creating parent directories automatically",
            is_mutating: true,
            is_core: true,
            parameters: vec![
                ("path", ParamSchema::req(String, "Target file path")),
                ("content", ParamSchema::req(String, "Full file content")),
            ],
        },
        ToolSpec {
            name: "replace_file_content",
            description: "Replace a target block of text in a file with whitespace-tolerant fuzzy self-healing",
            is_mutating: true,
            is_core: true,
            parameters: vec![
                ("path", ParamSchema::req(String, "Target file path")),
                ("target", ParamSchema::req(String, "Target text to replace")),
                ("replacement", ParamSchema::req(String, "Replacement text")),
            ],
        },
        ToolSpec {
            name: "multi_replace_file_content",
            description: "Apply multiple non-contiguous text replacements to a file atomically",
            is_mutating: true,
            is_core: true,
            parameters: vec![
                ("path", ParamSchema::req(String, "Target file path")),
                ("chunks", ParamSchema::req(Array, "Array of { target, replacement } objects")),
            ],
        },
        ToolSpec {
            name: "edit_file_anchored",
            description: "Apply FNV-1a hash-anchored line edit to a file",
            is_mutating: true,
            is_core: false,
            parameters: vec![
                ("path", ParamSchema::req(String, "Target file path")),
                ("line", ParamSchema::req(Integer, "1-indexed target line number")),
                (
                    "expectedAnchor",
                    ParamSchema::req(String, "Expected 8-char hex FNV-1a hash of the target line"),
                ),
                ("replacement", ParamSchema::req(String, "Replacement line content")),
            ],
        },
        ToolSpec {
            name: "apply_unified_diff",
            description: "Atomically apply a standard Unified Diff (--- a/, +++ b/) or V4A multi-file patch (*** Begin Patch)",
            is_mutating: true,
            is_core: true,
            parameters: vec![("diff", ParamSchema::req(String, "Unified diff or V4A patch text"))],
        },
        ToolSpec {
            name: "delete_file",
            description: "Delete a file on disk with transaction journal backup",
            is_mutating: true,
            is_core: true,
            parameters: vec![("path", ParamSchema::req(String, "Target file path to delete"))],
        },
        ToolSpec {
            name: "move_file",
            description: "Move or rename a file across the workspace",
            is_mutating: true,
            is_core: false,
            parameters: vec![
                ("source", ParamSchema::req(String, "Source file path")),
                ("target", ParamSchema::req(String, "Destination file path")),
            ],
        },
        ToolSpec {
            name: "list_dir",
            description: "List files and directories inside a directory",
            is_mutating: false,
            is_core: true,
            parameters: vec![(
                "path",
                ParamSchema::opt(String, "Directory path (defaults to workspace root)"),
            )],
        },
        ToolSpec {
            name: "find_files",
            description: "Find files matching a substring or glob pattern across the workspace",
            is_mutating: false,
            is_core: true,
            parameters: vec![
                ("query", ParamSchema::req(String, "File name substring or glob pattern")),
                ("path", ParamSchema::opt(String, "Base directory to search within")),
                ("maxDepth", ParamSchema::opt(Integer, "Maximum recursion depth (default 10)")),
            ],
        },
        ToolSpec {
            name: "file_info",
            description: "Get file metadata, size, line count, and SHA-256 digest",
            is_mutating: false,
            is_core: false,
            parameters: vec![("path", ParamSchema::req(String, "Target file path"))],
        },
        ToolSpec {
            name: "directory_tree",
            description: "Render a hierarchical tree view of a directory",
            is_mutating: false,
            is_core: false,
            parameters: vec![
                ("path", ParamSchema::opt(String, "Root directory path")),
                ("maxDepth", ParamSchema::opt(Integer, "Maximum depth (default 3)")),
            ],
        },
        ToolSpec {
            name: "grep_search",
            description: "Search file contents across a directory or file using regular expressions",
            is_mutating: false,
            is_core: true,
            parameters: vec![
                (
                    "query",
                    ParamSchema::req(String, "Regex pattern or literal text to search for"),
                ),
                ("path", ParamSchema::opt(String, "Directory or file path to search within")),
                (
                    "caseInsensitive",
                    ParamSchema::opt(Boolean, "Case-insensitive matching (default false)"),
                ),
                (
                    "maxResults",
                    ParamSchema::opt(Integer, "Maximum matches to return (default 100)"),
                ),
            ],
        },
        ToolSpec {
            name: "search_symbols",
            description: "Extract and search codebase symbols (fn, struct, enum, trait, class, interface, function, type, def) or file outline",
            is_mutating: false,
            is_core: true,
            parameters: vec![
                (
                    "query",
                    ParamSchema::opt(String, "Symbol name substring filter (omit for full file/workspace outline)"),
                ),
                ("path", ParamSchema::opt(String, "Specific file or subdirectory to inspect")),
                (
                    "kind",
                    ParamSchema::opt(
                        String,
                        "Optional symbol kind filter: function | struct | enum | trait | class | interface | type",
                    ),
                ),
                ("limit", ParamSchema::opt(Integer, "Maximum symbols to return (default 60)")),
            ],
        },
        ToolSpec {
            name: "batch_replace_regex",
            description: "Perform regex or symbol find-and-replace across workspace files with dry-run and rollback support",
            is_mutating: true,
            is_core: false,
            parameters: vec![
                ("target", ParamSchema::req(String, "Regex pattern or symbol name to find")),
                ("replacement", ParamSchema::req(String, "Replacement string")),
                ("path", ParamSchema::opt(String, "Subdirectory or file path (default: workspace root)")),
                (
                    "dryRun",
                    ParamSchema::opt(Boolean, "Preview matches without modifying files on disk"),
                ),
            ],
        },
        ToolSpec {
            name: "get_workspace_diff",
            description: "Generate unified diff of all file modifications recorded in the current turn or session",
            is_mutating: false,
            is_core: false,
            parameters: vec![("turnId", ParamSchema::opt(String, "Optional turn ID filter"))],
        },
        ToolSpec {
            name: "run_command",
            description: "Execute a shell command in the workspace with timeout and cancellation support",
            is_mutating: true,
            is_core: true,
            parameters: vec![
                ("command", ParamSchema::req(String, "Shell command line to execute")),
                ("cwd", ParamSchema::opt(String, "Working directory override")),
                (
                    "timeoutMs",
                    ParamSchema::opt(Integer, "Execution timeout in milliseconds (default 30000)"),
                ),
            ],
        },
        ToolSpec {
            name: "rollback_last_mutation",
            description: "Atomically rollback the most recent file mutation or all mutations in the current turn",
            is_mutating: true,
            is_core: true,
            parameters: vec![(
                "allInCurrentTurn",
                ParamSchema::opt(Boolean, "If true, rolls back all mutations in the current turn"),
            )],
        },
        ToolSpec {
            name: "save_memory",
            description: "Persist a key-value fact, rule, or architectural decision into session memory",
            is_mutating: false,
            is_core: true,
            parameters: vec![
                ("key", ParamSchema::req(String, "Unique memory key")),
                ("value", ParamSchema::req(String, "Memory content to store")),
                (
                    "category",
                    ParamSchema::opt(String, "Category: fact, rule, troubleshooting, or ki"),
                ),
            ],
        },
        ToolSpec {
            name: "search_memory",
            description: "Search persisted session memories by keyword or category",
            is_mutating: false,
            is_core: true,
            parameters: vec![("query", ParamSchema::opt(String, "Search query substring"))],
        },
        ToolSpec {
            name: "retrieve_spill_content",
            description: "Retrieve paginated line ranges from a truncated tool output spill ID",
            is_mutating: false,
            is_core: true,
            parameters: vec![
                (
                    "spillId",
                    ParamSchema::req(String, "Spill ID returned by truncated tool output"),
                ),
                ("startLine", ParamSchema::opt(Integer, "1-indexed start line")),
                ("endLine", ParamSchema::opt(Integer, "1-indexed end line")),
            ],
        },
        ToolSpec {
            name: "list_skills",
            description: "Discover available workspace SKILL.md manifests and instructions metadata",
            is_mutating: false,
            is_core: false,
            parameters: vec![],
        },
        ToolSpec {
            name: "use_skill",
            description: "Load the full SKILL.md instructions for a named workspace skill",
            is_mutating: false,
            is_core: false,
            parameters: vec![("name", ParamSchema::req(String, "Exact skill name to activate"))],
        },
    ]
}

