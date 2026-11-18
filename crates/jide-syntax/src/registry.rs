//! Language specification and detection registry for Just IDE.

use std::path::Path;

/// Metadata and syntax mapping for a supported programming language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSpec {
    /// Canonical internal identifier (e.g. "rust", "javascript").
    pub id: &'static str,
    /// Human-friendly display name (e.g. "Rust", "JavaScript").
    pub name: &'static str,
    /// Candidate syntax names or extensions recognized by `two-face` / `syntect`.
    pub syntect_names: &'static [&'static str],
    /// Line comment marker if supported by the language syntax.
    pub line_comment: Option<&'static str>,
    /// Block comment start and end delimiters if supported.
    pub block_comment: Option<(&'static str, &'static str)>,
    /// Characters that can trigger auto-indentation or scope adjustments.
    pub indent_triggers: &'static [char],
}

/// Static catalog of all 33 P0 languages defined in PRD §5.6.
pub static P0_LANGUAGES: [LanguageSpec; 33] = [
    LanguageSpec {
        id: "javascript",
        name: "JavaScript",
        syntect_names: &["JavaScript", "js"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{', ':', '['],
    },
    LanguageSpec {
        id: "typescript",
        name: "TypeScript",
        syntect_names: &["TypeScript", "ts"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{', ':', '['],
    },
    LanguageSpec {
        id: "jsx",
        name: "JSX",
        syntect_names: &["JSX", "JavaScript", "js"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{', '('],
    },
    LanguageSpec {
        id: "tsx",
        name: "TSX",
        syntect_names: &["TypeScriptReact", "tsx"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{', '('],
    },
    LanguageSpec {
        id: "json",
        name: "JSON",
        syntect_names: &["JSON", "json"],
        line_comment: None,
        block_comment: None,
        indent_triggers: &['{', '['],
    },
    LanguageSpec {
        id: "html",
        name: "HTML",
        syntect_names: &["HTML", "html"],
        line_comment: None,
        block_comment: Some(("<!--", "-->")),
        indent_triggers: &['>'],
    },
    LanguageSpec {
        id: "css",
        name: "CSS",
        syntect_names: &["CSS", "css"],
        line_comment: None,
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "scss",
        name: "SCSS",
        syntect_names: &["SCSS", "scss"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "python",
        name: "Python",
        syntect_names: &["Python", "py"],
        line_comment: Some("#"),
        block_comment: None,
        indent_triggers: &[':'],
    },
    LanguageSpec {
        id: "php",
        name: "PHP",
        syntect_names: &["PHP", "php"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "sql",
        name: "SQL",
        syntect_names: &["SQL", "sql"],
        line_comment: Some("--"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "rust",
        name: "Rust",
        syntect_names: &["Rust", "rs"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "markdown",
        name: "Markdown",
        syntect_names: &["Markdown", "md"],
        line_comment: None,
        block_comment: Some(("<!--", "-->")),
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "yaml",
        name: "YAML",
        syntect_names: &["YAML", "yaml", "yml"],
        line_comment: Some("#"),
        block_comment: None,
        indent_triggers: &[':'],
    },
    LanguageSpec {
        id: "toml",
        name: "TOML",
        syntect_names: &["TOML", "toml"],
        line_comment: Some("#"),
        block_comment: None,
        indent_triggers: &['['],
    },
    LanguageSpec {
        id: "xml",
        name: "XML",
        syntect_names: &["XML", "xml"],
        line_comment: None,
        block_comment: Some(("<!--", "-->")),
        indent_triggers: &['>'],
    },
    LanguageSpec {
        id: "shell",
        name: "Shell/Bash",
        syntect_names: &["Bourne Again Shell (bash)", "sh", "bash"],
        line_comment: Some("#"),
        block_comment: None,
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "powershell",
        name: "PowerShell",
        syntect_names: &["PowerShell", "Bourne Again Shell (bash)", "sh"],
        line_comment: Some("#"),
        block_comment: Some(("<#", "#>")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "batch",
        name: "Batch",
        syntect_names: &["Batch File", "bat", "cmd"],
        line_comment: Some("REM "),
        block_comment: None,
        indent_triggers: &['('],
    },
    LanguageSpec {
        id: "c",
        name: "C",
        syntect_names: &["C", "c"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "cpp",
        name: "C++",
        syntect_names: &["C++", "cpp"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "csharp",
        name: "C#",
        syntect_names: &["C#", "cs"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "java",
        name: "Java",
        syntect_names: &["Java", "java"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "go",
        name: "Go",
        syntect_names: &["Go", "go"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "kotlin",
        name: "Kotlin",
        syntect_names: &["Kotlin", "kt"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        indent_triggers: &['{'],
    },
    LanguageSpec {
        id: "ruby",
        name: "Ruby",
        syntect_names: &["Ruby", "rb"],
        line_comment: Some("#"),
        block_comment: Some(("=begin", "=end")),
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "lua",
        name: "Lua",
        syntect_names: &["Lua", "lua"],
        line_comment: Some("--"),
        block_comment: Some(("--[[", "]]")),
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "dockerfile",
        name: "Dockerfile",
        syntect_names: &["Dockerfile"],
        line_comment: Some("#"),
        block_comment: None,
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "ini",
        name: "INI",
        syntect_names: &["INI", "ini"],
        line_comment: Some(";"),
        block_comment: None,
        indent_triggers: &['['],
    },
    LanguageSpec {
        id: "makefile",
        name: "Makefile",
        syntect_names: &["Makefile"],
        line_comment: Some("#"),
        block_comment: None,
        indent_triggers: &[':'],
    },
    LanguageSpec {
        id: "diff",
        name: "Diff",
        syntect_names: &["Diff", "diff"],
        line_comment: None,
        block_comment: None,
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "log",
        name: "Log",
        syntect_names: &["log"],
        line_comment: None,
        block_comment: None,
        indent_triggers: &[],
    },
    LanguageSpec {
        id: "plaintext",
        name: "Plain Text",
        syntect_names: &["Plain Text", "txt"],
        line_comment: None,
        block_comment: None,
        indent_triggers: &[],
    },
];

/// Returns all 33 P0 language specifications.
#[must_use]
pub fn all_p0_languages() -> &'static [LanguageSpec] {
    &P0_LANGUAGES
}

/// Finds a P0 language specification by its canonical id (e.g. "rust").
#[must_use]
pub fn find_language_by_id(id: &str) -> Option<&'static LanguageSpec> {
    P0_LANGUAGES
        .iter()
        .find(|spec| spec.id.eq_ignore_ascii_case(id))
}

/// Detects the language specification from path and optional first line content.
///
/// Priority:
/// 1. File extension
/// 2. Exact file name
/// 3. Shebang in first line
/// 4. Fallback to Plain Text
#[must_use]
pub fn detect_language(path: Option<&Path>, first_line: Option<&str>) -> &'static LanguageSpec {
    if let Some(p) = path {
        if let Some(spec) = detect_by_extension(p) {
            return spec;
        }

        if let Some(spec) = detect_by_exact_filename(p) {
            return spec;
        }
    }

    if let Some(line) = first_line {
        if let Some(spec) = detect_by_shebang(line) {
            return spec;
        }
    }

    fallback_plaintext()
}

fn fallback_plaintext() -> &'static LanguageSpec {
    // Plain Text is the last item in P0_LANGUAGES
    &P0_LANGUAGES[32]
}

fn detect_by_extension(path: &Path) -> Option<&'static LanguageSpec> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();

    let target_id = match ext.as_str() {
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "json" => "json",
        "html" | "htm" | "xhtml" => "html",
        "css" => "css",
        "scss" => "scss",
        "py" | "pyw" | "pyi" => "python",
        "php" | "phtml" => "php",
        "sql" => "sql",
        "rs" => "rust",
        "md" | "markdown" => "markdown",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" | "svg" | "xaml" => "xml",
        "sh" | "bash" | "zsh" => "shell",
        "ps1" | "psm1" | "psd1" => "powershell",
        "bat" | "cmd" => "batch",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" => "csharp",
        "java" => "java",
        "go" => "go",
        "kt" | "kts" => "kotlin",
        "rb" | "rake" => "ruby",
        "lua" => "lua",
        "ini" | "cfg" => "ini",
        "diff" | "patch" => "diff",
        "log" => "log",
        "txt" | "text" => "plaintext",
        _ => return None,
    };

    find_language_by_id(target_id)
}

fn detect_by_exact_filename(path: &Path) -> Option<&'static LanguageSpec> {
    let name = path.file_name()?.to_str()?;

    let target_id = if name.eq_ignore_ascii_case("Dockerfile") || name.starts_with(".Dockerfile") {
        "dockerfile"
    } else if name.eq_ignore_ascii_case("Makefile") || name == "GNUmakefile" {
        "makefile"
    } else if name == ".bashrc"
        || name == ".bash_profile"
        || name == ".bash_aliases"
        || name == ".zshrc"
        || name == ".profile"
    {
        "shell"
    } else if name == ".editorconfig" || name == ".gitconfig" {
        "ini"
    } else if name == "Cargo.lock" {
        "toml"
    } else {
        return None;
    };

    find_language_by_id(target_id)
}

fn detect_by_shebang(first_line: &str) -> Option<&'static LanguageSpec> {
    let trimmed = first_line.trim_start();
    if !trimmed.starts_with("#!") {
        return None;
    }

    let line_lower = trimmed.to_ascii_lowercase();
    let target_id = if line_lower.contains("python") {
        "python"
    } else if line_lower.contains("bash")
        || line_lower.contains("/sh")
        || line_lower.contains("zsh")
        || line_lower.contains("/ash")
    {
        "shell"
    } else if line_lower.contains("node")
        || line_lower.contains("deno")
        || line_lower.contains("bun")
    {
        "javascript"
    } else if line_lower.contains("pwsh") || line_lower.contains("powershell") {
        "powershell"
    } else if line_lower.contains("ruby") {
        "ruby"
    } else if line_lower.contains("php") {
        "php"
    } else if line_lower.contains("lua") {
        "lua"
    } else {
        return None;
    };

    find_language_by_id(target_id)
}
