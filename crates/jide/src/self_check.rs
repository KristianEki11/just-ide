use jide_core::{DocId, Document, SaveFault, Selection};
use serde::Serialize;
use std::time::Instant;

/// JSON output payload for `jide --self-check` (`02-TECH-SPEC.md` §10).
#[derive(Debug, Serialize)]
pub struct SelfCheckReport {
    /// Overall health status.
    pub ok: bool,
    /// Headless self-check initialization time in ms.
    pub startup_ms: u64,
    /// Core buffer check outcome.
    pub buffer_ops: &'static str,
    /// Count of verified P0 syntax languages.
    pub highlight_langs_ok: usize,
    /// Languages that failed syntax verification.
    pub highlight_langs_failed: Vec<String>,
    /// Search engine check outcome.
    pub search: &'static str,
    /// Atomic save smoke check outcome.
    pub atomic_save: &'static str,
    /// Total elapsed check time in ms.
    pub elapsed_ms: u64,
}

/// Executes headless sanity checks and returns the structured report.
pub fn run(process_start: Instant) -> SelfCheckReport {
    let startup_ms = u64::try_from(process_start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let check_start = Instant::now();

    let buffer_ok = jide_core::self_check_buffer();
    let search_ok = jide_search::self_check_search();
    let (highlight_langs_ok, highlight_langs_failed) = check_p0_languages();
    let themes_ok = check_themes();
    let atomic_ok = check_atomic_temp_save().is_ok();

    let ok = buffer_ok
        && search_ok
        && themes_ok
        && atomic_ok
        && highlight_langs_failed.is_empty()
        && highlight_langs_ok == jide_syntax::planned_p0_language_count();
    let elapsed_ms = u64::try_from(check_start.elapsed().as_millis()).unwrap_or(u64::MAX);

    SelfCheckReport {
        ok,
        startup_ms,
        buffer_ops: if buffer_ok { "ok" } else { "failed" },
        highlight_langs_ok,
        highlight_langs_failed,
        search: if search_ok { "ok" } else { "failed" },
        atomic_save: if atomic_ok { "ok" } else { "failed" },
        elapsed_ms,
    }
}

fn check_p0_languages() -> (usize, Vec<String>) {
    let syntax_set = jide_syntax::load_syntax_set();
    let mut ok = 0usize;
    let mut failed = Vec::new();
    for spec in jide_syntax::all_p0_languages() {
        let snippet = sample_snippet_for_lang(spec.id);
        let mut doc = Document::from_text(DocId(100), snippet);
        doc.language = jide_core::LanguageId::new(spec.id);
        let mut cache = jide_syntax::HighlightCache::new(spec.id);
        let highlighted = cache.highlight_visible(&syntax_set, &doc, 0..doc.line_count());
        let syntax_resolved = spec.syntect_names.is_empty()
            || spec
                .syntect_names
                .iter()
                .any(|name| syntax_set.find_syntax_by_name(name).is_some());
        let spans_non_empty = highlighted.iter().any(|line_spans| !line_spans.is_empty());
        let semantic_token = spec.syntect_names.is_empty()
            || highlighted.iter().flatten().any(|span| {
                span.token != jide_syntax::SyntaxToken::Default
                    || spec.id == "plaintext"
                    || spec.id == "log"
                    || spec.id == "diff"
            });
        if syntax_resolved && spans_non_empty && semantic_token {
            ok += 1;
        } else {
            failed.push(spec.id.to_owned());
        }
    }
    (ok, failed)
}

fn sample_snippet_for_lang(lang_id: &str) -> &'static str {
    match lang_id {
        "rust" => "pub fn main() { let x = 42; }",
        "javascript" => "export const x = 'hi'; // c",
        "typescript" => "interface A { x: number }",
        "jsx" => "const el = <div id=\"a\">{1}</div>;",
        "tsx" => "const el: any = <Comp x={1} />;",
        "json" => "{\"key\": 123, \"ok\": true}",
        "html" => "<div class=\"box\">hello</div>",
        "css" => "body { color: #fff; margin: 0; }",
        "scss" => "$fg: #fff; body { color: $fg; }",
        "python" => "def f(x): return x + 1 # c",
        "php" => "<?php $x = 'hi'; echo $x;",
        "sql" => "SELECT id, name FROM users WHERE id = 1;",
        "markdown" => "# Heading\n**bold** `code`",
        "yaml" => "name: just-ide\ncount: 42",
        "toml" => "[package]\nname = \"jide\"",
        "xml" => "<root attr=\"1\">text</root>",
        "shell" => "#!/bin/sh\necho \"hello\"",
        "powershell" => "function Test-Run { Write-Output \"hi\" }",
        "batch" => "@echo off\nSET X=1",
        "c" => "#include <stdio.h>\nint x = 1;",
        "cpp" => "class Foo { public: int x = 1; };",
        "csharp" => "public class App { static int X = 1; }",
        "java" => "public class Main { int x = 1; }",
        "go" => "package main\nfunc main() { var x = 1 }",
        "kotlin" => "fun main() { val x: Int = 1 }",
        "ruby" => "def hello; puts 'hi'; end",
        "lua" => "local function f(x) return x + 1 end",
        "dockerfile" => "FROM rust:1.80\nRUN echo ok",
        "ini" => "[section]\nkey = value",
        "makefile" => "all:\n\techo building",
        "diff" => "--- a/f\n+++ b/f\n+added line",
        "log" => "2026-01-01 INFO started",
        _ => "plain text sample line",
    }
}

fn check_themes() -> bool {
    let dark = jide_ui::theme::Theme::builtin_dark();
    let light = jide_ui::theme::Theme::builtin_light();
    let hc = jide_ui::theme::Theme::builtin_high_contrast();
    !dark.name.is_empty() && !light.name.is_empty() && !hc.name.is_empty()
}

fn check_atomic_temp_save() -> anyhow::Result<()> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("jide-self-check-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&dir)?;
    let target = dir.join("roundtrip.txt");
    let result = (|| -> anyhow::Result<()> {
        let original = "just-ide 😀 e\u{301}\r\n";
        let mut doc = Document::from_bytes(DocId(1), None, original.as_bytes())?;
        doc.selection = Selection::point(0);
        doc.insert_text("checked ", 1000)?;
        anyhow::ensure!(doc.undo()?);
        anyhow::ensure!(doc.redo()?);
        doc.save_to(&target, SaveFault::None)?;
        anyhow::ensure!(!doc.is_dirty());
        anyhow::ensure!(std::fs::read(&target)? == format!("checked {original}").as_bytes());
        let loaded = Document::open(DocId(2), &target)?;
        anyhow::ensure!(loaded.rope == doc.rope);
        doc.insert_char('!', 2000)?;
        doc.save_to(&target, SaveFault::None)?;
        anyhow::ensure!(Document::open(DocId(3), &target)?.rope == doc.rope);
        Ok(())
    })();
    let cleanup = std::fs::remove_file(&target);
    let remove_dir = std::fs::remove_dir(&dir);
    result?;
    cleanup?;
    remove_dir?;
    Ok(())
}
