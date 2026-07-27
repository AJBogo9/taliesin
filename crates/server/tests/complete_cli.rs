//! The two halves of shell completion, driven through the real binary.
//!
//! The hidden `__complete` subcommand is the runtime brain: it prints candidate lines then a
//! trailing `:<directive>` line, and these tests invoke it the way a shim does, so they cover
//! dispatch wiring + the wire protocol end to end.
//!
//! `completions <shell> [--install]` is the setup half, and it is the one that **writes to
//! the user's home directory** — the only place this tool installs anything. A subprocess is
//! the honest harness for it: the install path reads `$HOME`/`$XDG_DATA_HOME` and the exit
//! code is the contract, and both can be controlled here without touching this process's own
//! environment.

use std::process::Command;

/// Run `taliesin __complete <words…>` in `cwd`, returning (candidate values, directive).
fn complete(cwd: &std::path::Path, words: &[&str]) -> (Vec<String>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("__complete")
        .args(words)
        .current_dir(cwd)
        .output()
        .expect("run taliesin __complete");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    let mut values = Vec::new();
    let mut directive = String::new();
    for line in text.lines() {
        if let Some(d) = line.strip_prefix(':') {
            directive = d.to_string();
        } else {
            values.push(line.split('\t').next().unwrap_or("").to_string());
        }
    }
    (values, directive)
}

#[test]
fn completes_subcommands_and_suppresses_files() {
    let (values, directive) = complete(std::path::Path::new("."), &[""]);
    assert!(
        values.contains(&"preview".to_string()),
        "offers preview: {values:?}"
    );
    assert!(
        values.contains(&"build".to_string()),
        "offers build: {values:?}"
    );
    assert_eq!(directive, "4", "NoFileComp for subcommand completion");
}

#[test]
fn path_completion_end_to_end_filters_dirs() {
    use std::fs;
    let dir = std::env::temp_dir().join(format!("tali-complete-e2e-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("site")).unwrap();
    fs::create_dir_all(dir.join("target")).unwrap();
    fs::write(dir.join("index.tmd"), "# hi\n").unwrap();
    fs::write(dir.join("site/_site.yml"), "title: S\n").unwrap();
    fs::write(dir.join("site/page.tmd"), "# p\n").unwrap();
    fs::write(dir.join("target/decoy.tmd"), "# d\n").unwrap();

    let (values, directive) = complete(&dir, &["preview", ""]);
    assert!(
        values.contains(&"index.tmd".to_string()),
        "offers .tmd: {values:?}"
    );
    assert!(
        values.contains(&"site/".to_string()),
        "offers site dir: {values:?}"
    );
    assert!(
        !values.iter().any(|v| v.starts_with("target")),
        "hides target/: {values:?}"
    );
    assert_eq!(directive, "5", "NoSpace|NoFileComp when dirs present");
    let _ = fs::remove_dir_all(&dir);
}

/// Run `taliesin completions <args…>` against a throwaway `$HOME`, returning
/// (exit code, stdout, stderr). `XDG_DATA_HOME` is cleared so the install path takes its
/// documented default under that home rather than the developer's own.
fn completions(home: &std::path::Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("completions")
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("run taliesin completions");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-completions-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp home");
    dir
}

/// Naming a shell PRINTS the script; it must not install anything. The two modes are one
/// character apart in the argument scan, and the wrong branch writes a file into the user's
/// home directory when they only asked to look at it.
#[test]
fn naming_a_shell_prints_the_script_and_installs_nothing() {
    let home = temp_home("print");
    let (code, stdout, stderr) = completions(&home, &["zsh"]);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert!(
        stdout.contains("__complete") && stdout.contains("compdef"),
        "expected the zsh shim on stdout, got {stdout:.200?}"
    );
    let installed = home.join(".local/share/zsh/site-functions/_taliesin");
    assert!(
        !installed.exists(),
        "printing a script must not write {}",
        installed.display()
    );
    let _ = std::fs::remove_dir_all(&home);
}

/// `--install` writes the script where that shell reads completions from, and reports where.
/// This is the only path in the tool that creates a file outside the project, and nothing
/// exercised it end to end: `install_plan` was unit-tested on a hand-built environment, so
/// the step that reads the REAL environment and performs the write was never run.
#[test]
fn install_writes_the_script_under_the_resolved_home() {
    let home = temp_home("install");
    let (code, _stdout, stderr) = completions(&home, &["zsh", "--install"]);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    let installed = home.join(".local/share/zsh/site-functions/_taliesin");
    let written = std::fs::read_to_string(&installed)
        .unwrap_or_else(|e| panic!("expected {} ({e}); stderr: {stderr}", installed.display()));
    assert!(
        written.contains("__complete"),
        "the installed file should be the shim, got {written:.200?}"
    );
    assert!(
        stderr.contains(&installed.display().to_string()),
        "the tool should say where it landed, got {stderr:?}"
    );
    // The flag may lead or follow the shell name; both must reach the same place.
    let home2 = temp_home("install2");
    let (code2, _, stderr2) = completions(&home2, &["--install", "zsh"]);
    assert_eq!(code2, Some(0), "stderr: {stderr2}");
    assert!(
        home2
            .join(".local/share/zsh/site-functions/_taliesin")
            .exists(),
        "`--install zsh` must install too, got {stderr2:?}"
    );
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&home2);
}

/// Both failure modes exit non-zero, which is the whole reason the command returns a code:
/// a shell's install snippet runs this unattended, so "succeeded and did nothing" is the
/// outcome a user never finds out about.
#[test]
fn an_unusable_request_exits_nonzero() {
    let home = temp_home("fail");
    // No shell named and none to print.
    let (code, stdout, _) = completions(&home, &[]);
    assert_eq!(code, Some(1), "a missing shell is a usage error");
    assert!(stdout.is_empty(), "nothing to print, got {stdout:?}");
    // A shell that does not exist, on the install path.
    let (code, _, stderr) = completions(&home, &["frobnicate", "--install"]);
    assert_eq!(code, Some(1), "an unknown shell cannot be installed");
    assert!(
        stderr.contains("frobnicate"),
        "the error should name the shell, got {stderr:?}"
    );
    let _ = std::fs::remove_dir_all(&home);
}
