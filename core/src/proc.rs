//! Child processes that run without a window of their own.
//!
//! The Windows GUI is built for the `windows` subsystem and has no console, so a console program
//! it starts (`sbx`, `aisbx`, `git`) would open a console window of its own, once per start. Every
//! worker and refresh runs such programs, so they all start through [`command`]. A terminal the
//! user asked for is the exception: `terminal` opens that one on purpose.

use std::ffi::OsStr;
use std::process::Command;

/// A [`Command`] for `program` that opens no console window on Windows, and is a plain
/// `Command::new` elsewhere.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    no_window(&mut command);
    command
}

#[cfg(windows)]
fn no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_runs_the_program_with_its_arguments() {
        let program = std::env::current_exe().unwrap();
        let out = command(&program).arg("--list").output().unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("tests::"));
    }

    #[test]
    fn no_spawn_outside_the_tests_bypasses_the_helper() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&src).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.extension().is_none_or(|e| e != "rs") || name == "proc.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let code = text.split("\n#[cfg(test)]").next().unwrap();
            for (i, line) in code.lines().enumerate() {
                let terminal = name == "terminal.rs"
                    && line.trim() == "let mut command = Command::new(program);";
                if line.contains("Command::new(")
                    && !line.trim_start().starts_with("//")
                    && !terminal
                {
                    offenders.push(format!("{name}:{}: {}", i + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "start these through proc::command:\n{}",
            offenders.join("\n")
        );
    }
}
