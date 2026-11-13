use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn legacy_machine(text: &str) -> bool {
    text.to_ascii_lowercase()
        .as_bytes()
        .windows(5)
        .any(|bytes| bytes.starts_with(b"neko") && bytes[4].is_ascii_digit())
}

fn check_portability(directory: &Path, base: &Path, failures: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base).unwrap();
        if [".git", "target", "artifacts", "dist"]
            .contains(&entry.file_name().to_string_lossy().as_ref())
            || entry.file_name() == "result"
            || entry.file_name().to_string_lossy().starts_with("result-")
        {
            continue;
        }
        if entry.file_type()?.is_symlink() {
            failures.push(format!(
                "unexpected repository symlink {}",
                relative.display()
            ));
        } else if path.is_dir() {
            check_portability(&path, base, failures)?;
        } else {
            let bytes = fs::read(&path)?;
            let text = String::from_utf8_lossy(&bytes);
            if legacy_machine(&relative.to_string_lossy()) || legacy_machine(&text) {
                failures.push(format!("legacy machine alias in {}", relative.display()));
            }
            if !relative.starts_with("docs/history")
                && ["rs", "nix", "toml", "json", "sh"].contains(
                    &path
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                )
                && (text.contains(&["/", "home/"].concat())
                    || text.contains(&["/", "nix/store/"].concat()))
            {
                failures.push(format!(
                    "machine-local path in runnable file {}",
                    relative.display()
                ));
            }
        }
    }
    Ok(())
}

fn verify(base: &Path) -> io::Result<bool> {
    let runs = base.join("artifacts/verification");
    let run = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos()
        .to_string();
    let logs = runs.join("runs").join(&run);
    fs::create_dir_all(&logs)?;
    let checks: &[(&str, &[&str])] = &[
        ("fmt", &["fmt", "--all", "--", "--check"]),
        (
            "check",
            &[
                "check",
                "--workspace",
                "--all-targets",
                "--locked",
                "--offline",
            ],
        ),
        (
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--offline",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("tests", &["test", "--workspace", "--locked", "--offline"]),
        (
            "doc-tests",
            &["test", "--workspace", "--doc", "--locked", "--offline"],
        ),
        (
            "no-default-features",
            &[
                "test",
                "--workspace",
                "--no-default-features",
                "--locked",
                "--offline",
            ],
        ),
        (
            "all-features",
            &[
                "check",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--offline",
            ],
        ),
        (
            "wasm",
            &[
                "check",
                "-p",
                "splitcast-web",
                "--target",
                "wasm32-unknown-unknown",
                "--locked",
                "--offline",
            ],
        ),
        (
            "duplicate-dependencies",
            &["tree", "--duplicates", "--locked", "--offline"],
        ),
        (
            "dependency-features",
            &["tree", "--edges", "features", "--locked", "--offline"],
        ),
    ];
    let mut summary = String::from("check\tresult\texit_code\tcommand\n");
    let mut passed = true;
    for (name, args) in checks {
        let result = Command::new("cargo").args(*args).current_dir(base).output();
        let (success, code, output) = match result {
            Ok(output) => {
                let mut bytes = output.stdout;
                bytes.extend(output.stderr);
                (
                    output.status.success(),
                    output
                        .status
                        .code()
                        .map_or_else(|| "signal".into(), |code| code.to_string()),
                    bytes,
                )
            }
            Err(error) => (false, "not-started".into(), error.to_string().into_bytes()),
        };
        fs::write(logs.join(format!("{name}.log")), output)?;
        let status = if success { "PASS" } else { "FAIL" };
        println!("{name}: {status}");
        summary.push_str(&format!(
            "{name}\t{status}\t{code}\tcargo {}\n",
            args.join(" ")
        ));
        passed &= success;
    }
    let mut failures = Vec::new();
    check_portability(base, base, &mut failures)?;
    fs::write(logs.join("portability.log"), failures.join("\n"))?;
    let status = if failures.is_empty() { "PASS" } else { "FAIL" };
    summary.push_str(&format!(
        "portability\t{status}\t{}\tsplitcast-xtask portability\n",
        usize::from(!failures.is_empty())
    ));
    println!("portability: {status}");
    fs::write(logs.join("summary.tsv"), summary)?;
    fs::write(runs.join("latest-run.txt"), format!("runs/{run}\n"))?;
    println!("evidence: {}", logs.strip_prefix(base).unwrap().display());
    Ok(passed && failures.is_empty())
}

fn run() -> io::Result<bool> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let base = root();
    match args.as_slice() {
        [command] if command == "verify" => verify(&base),
        [command] if command == "portability" => {
            let mut failures = Vec::new();
            check_portability(&base, &base, &mut failures)?;
            for failure in &failures {
                eprintln!("{failure}");
            }
            Ok(failures.is_empty())
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: splitcast-xtask verify | portability",
        )),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn role_names_are_portable_and_numbered_legacy_aliases_are_not() {
        assert!(!legacy_machine("capture-host production-host"));
        assert!(legacy_machine(&format!("{}42", "neko")));
    }
}
