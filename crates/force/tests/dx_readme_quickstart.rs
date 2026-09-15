//! Onramp clean-room check for the top-level `README.md` "first run" journey.
//!
//! `README.md` is what crates.io and GitHub actually render for a brand-new
//! developer (`readme = "../../README.md"` in this crate's `Cargo.toml`) — it
//! is the front door, not just another doc page. These tests extract the
//! literal `Installation` + `Quick Start` content from `README.md` at test
//! time (never a hand-maintained copy that can silently drift from the real
//! docs) and prove two things a developer relies on without ever checking
//! them: that the pinned dependency version actually resolves to something
//! that exists in this release, and that the Quick Start code compiles
//! exactly as shown, from a throwaway project that has nothing but the
//! extracted snippet in it.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repo root from crate manifest dir")
}

fn read_readme() -> String {
    fs::read_to_string(repo_root().join("README.md")).expect("read README.md")
}

/// Text between `## {heading}` and the next top-level `## ` heading.
fn section<'a>(readme: &'a str, heading: &str) -> &'a str {
    let marker = format!("## {heading}");
    let start = readme
        .find(&marker)
        .unwrap_or_else(|| panic!("README.md: missing '## {heading}' section"))
        + marker.len();
    let rest = &readme[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    &rest[..end]
}

fn fenced_block<'a>(text: &'a str, lang: &str) -> &'a str {
    let fence = format!("```{lang}\n");
    let start = text
        .find(&fence)
        .unwrap_or_else(|| panic!("README.md: missing ```{lang} fenced block in section"))
        + fence.len();
    let rest = &text[start..];
    let end = rest
        .find("```")
        .unwrap_or_else(|| panic!("README.md: unterminated ```{lang} fenced block"));
    &rest[..end]
}

/// The literal, pastable `[dependencies]` lines from `## Installation`.
///
/// The Installation block also shows an illustrative "enable specific
/// features" variant behind a `# Or enable specific features:` comment, with
/// a second `force = {...}` line — a literal copy-paste of the whole fenced
/// block would redeclare the `force` key twice and fail to parse as TOML.
/// We stop at that comment, matching what a reader who wants "just add the
/// dependency" actually pastes.
fn installation_deps(readme: &str) -> String {
    let block = fenced_block(section(readme, "Installation"), "toml");
    block
        .lines()
        .take_while(|line| {
            !line
                .trim_start()
                .starts_with("# Or enable specific features")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn quickstart_code(readme: &str) -> &str {
    fenced_block(section(readme, "Quick Start"), "rust")
}

/// Replace the `force = ...` dependency line with a path dependency pointing
/// at this checkout, so the test proves "does the snippet in the docs
/// compile against the code we are about to ship", not against whatever is
/// currently on crates.io.
fn point_force_dependency_at_checkout(deps: &str, force_crate_path: &Path) -> String {
    let replacement = format!(
        "force = {{ path = {:?} }}",
        force_crate_path
            .to_str()
            .expect("checkout path is valid UTF-8")
    );
    deps.lines()
        .map(|line| {
            if line.trim_start().starts_with("force") && line.contains('=') {
                replacement.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Minimal Cargo caret-requirement range for the plain `force = "X.Y[.Z]"`
/// shapes actually used in this README (no `^`/`~`/comparison operators).
fn caret_range(requirement: &str) -> ((u64, u64, u64), (u64, u64, u64)) {
    let mut parts = requirement
        .split('.')
        .map(|p| p.parse::<u64>().expect("numeric version component"));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    let lower = (major, minor, patch);
    let upper = if major > 0 {
        (major + 1, 0, 0)
    } else if minor > 0 {
        (0, minor + 1, 0)
    } else {
        (0, 0, patch + 1)
    };
    (lower, upper)
}

fn admits(version: (u64, u64, u64), requirement: &str) -> bool {
    let (lower, upper) = caret_range(requirement);
    lower <= version && version < upper
}

fn current_version() -> (u64, u64, u64) {
    let mut parts = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|p| p.parse::<u64>().expect("numeric crate version component"));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Every `force = "X.Y"` / `force = { version = "X.Y", ... }` pin in
/// `README.md`, as `(1-based line number, requirement string)`.
fn find_force_version_pins(readme: &str) -> Vec<(usize, String)> {
    let mut pins = Vec::new();
    for (idx, line) in readme.lines().enumerate() {
        let mut rest = line;
        while let Some(pos) = rest.find("force") {
            rest = &rest[pos + "force".len()..];
            let Some(eq) = rest.find('=') else { continue };
            let after_eq = &rest[eq + 1..];
            let Some(quote_start) = after_eq.find('"') else {
                continue;
            };
            let after_quote = &after_eq[quote_start + 1..];
            let Some(quote_end) = after_quote.find('"') else {
                continue;
            };
            let requirement = &after_quote[..quote_end];
            if requirement.chars().all(|c| c.is_ascii_digit() || c == '.')
                && !requirement.is_empty()
            {
                pins.push((idx + 1, requirement.to_string()));
            }
        }
    }
    pins
}

#[test]
fn readme_version_pins_admit_the_current_release() {
    let readme = read_readme();
    let current = current_version();
    let current_str = env!("CARGO_PKG_VERSION");

    let stale: Vec<String> = find_force_version_pins(&readme)
        .into_iter()
        .filter(|(_, requirement)| !admits(current, requirement))
        .map(|(lineno, requirement)| {
            format!("README.md:{lineno}: force = \"{requirement}\" does not match {current_str}")
        })
        .collect();

    assert!(
        stale.is_empty(),
        "README.md pins a `force` version that does not admit the current release ({current_str}). \
         This resolves silently to a real, old, non-yanked crate on crates.io instead of erroring, \
         so it never breaks on its own -- fix the pin(s):\n{}",
        stale.join("\n")
    );
}

/// Ignored like the live-contract tests (see `tests/README.md`): it spawns a
/// real `cargo build` for a freshly generated scratch crate with no
/// `Cargo.lock`, which can hit the network to resolve/download
/// dependencies. Outer `--offline`/`--frozen` flags don't propagate to that
/// nested invocation, so keeping this on by default would break the
/// documented guarantee (`docs/guide/04-live-contract-testing.md`) that the
/// default `cargo test` stays fully hermetic. Run explicitly with
/// `cargo test -p force --test dx_readme_quickstart --all-features -- --ignored`.
#[test]
#[ignore = "spawns `cargo build` for a scratch crate, which can touch the network"]
fn readme_quickstart_compiles_from_a_clean_room() {
    let readme = read_readme();
    let deps = installation_deps(&readme);
    let code = quickstart_code(&readme);
    let force_crate = repo_root().join("crates/force");
    let deps = point_force_dependency_at_checkout(&deps, &force_crate);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let scratch = std::env::temp_dir().join(format!(
        "onramp-readme-quickstart-{}-{nanos}",
        std::process::id()
    ));
    let src_dir = scratch.join("src");
    fs::create_dir_all(&src_dir).expect("create clean-room scratch project");

    let manifest = format!(
        "[package]\nname = \"onramp-quickstart-check\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n{deps}\n"
    );
    fs::write(scratch.join("Cargo.toml"), manifest).expect("write scratch Cargo.toml");
    fs::write(src_dir.join("main.rs"), code).expect("write scratch main.rs");

    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .arg("build")
        .current_dir(&scratch)
        .output()
        .expect("run `cargo build` in the clean-room scratch project");

    let _ = fs::remove_dir_all(&scratch);

    assert!(
        output.status.success(),
        "README.md Quick Start does not compile from a clean room \
         (copy-pasted verbatim into a fresh project against this checkout).\n\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
