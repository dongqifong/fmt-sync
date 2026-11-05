use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Output,
};

use anyhow::Result;

use crate::{
    execution::{StageKind, StageResult, Status},
    language::Language,
    tools::{Tool, append},
};

#[derive(Clone, Copy)]
pub enum StdoutPolicy {
    Diagnostics,
    Findings,
}

#[derive(Clone, Copy)]
pub struct OutcomePolicy {
    pub finding_codes: &'static [i32],
    pub stdout: StdoutPolicy,
}

impl OutcomePolicy {
    pub const fn findings(finding_codes: &'static [i32]) -> Self {
        Self {
            finding_codes,
            stdout: StdoutPolicy::Diagnostics,
        }
    }
}

pub struct StageCommand<'a> {
    pub language: Language,
    pub stage: StageKind,
    pub tool: &'a Tool,
    pub args: &'a [OsString],
    pub policy: OutcomePolicy,
}

pub fn run(root: &Path, command: &StageCommand<'_>) -> StageResult {
    let (status, detail) = evaluate(command.tool.output(root, command.args), command.policy);
    result(command, status, detail)
}

pub fn run_batched(root: &Path, command: &StageCommand<'_>, paths: &[PathBuf]) -> StageResult {
    let mut status = Status::Passed;
    let mut details = String::new();
    for paths in batches(paths) {
        let output = command.tool.output(root, &append(command.args, &paths));
        let (next, detail) = evaluate(output, command.policy);
        if next.code() > status.code() {
            status = next;
        }
        details.push_str(&detail);
    }
    result(command, status, details)
}

fn result(command: &StageCommand<'_>, status: Status, detail: String) -> StageResult {
    StageResult {
        language: command.language,
        stage: command.stage,
        status,
        tool: command.tool.label.clone(),
        detail,
    }
}

fn evaluate(output: Result<Output>, policy: OutcomePolicy) -> (Status, String) {
    let output = match output {
        Ok(output) => output,
        Err(error) => return (Status::Error, format!("{error:#}\n")),
    };
    let code = output.status.code();
    let status = if output.status.success() {
        if matches!(policy.stdout, StdoutPolicy::Findings) && !output.stdout.is_empty() {
            Status::Failed
        } else {
            Status::Passed
        }
    } else if code.is_some_and(|code| policy.finding_codes.contains(&code)) {
        Status::Failed
    } else {
        Status::Error
    };
    let mut detail = String::from_utf8_lossy(&output.stdout).to_string();
    detail.push_str(&String::from_utf8_lossy(&output.stderr));
    if status != Status::Passed && detail.trim().is_empty() {
        detail.push_str(&format!("Tool exit code: {code:?}\n"));
    }
    (status, detail)
}

// A conservative bound leaves room for UTF-16 expansion, quoting, and the executable path.
pub fn batches(items: &[PathBuf]) -> Vec<Vec<OsString>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut size = 0;
    for item in items {
        let arg = Path::new(".").join(item).into_os_string();
        let cost = arg.to_string_lossy().encode_utf16().count() * 2 + 4;
        if size + cost > 6000 && !current.is_empty() {
            batches.push(std::mem::take(&mut current));
            size = 0;
        }
        size += cost;
        current.push(arg);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::ExitStatus;

    fn output(code: u32, stdout: &[u8], stderr: &[u8]) -> Result<Output> {
        #[cfg(windows)]
        let status = {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(code)
        };
        #[cfg(unix)]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw((code as i32) << 8)
        };
        Ok(Output {
            status,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        })
    }

    #[test]
    fn finding_codes_are_explicit_and_preserve_diagnostics() {
        for (code, codes, expected) in [
            (0, &[][..], Status::Passed),
            (1, &[1][..], Status::Failed),
            (2, &[1][..], Status::Error),
            (101, &[1][..], Status::Error),
            (101, &[1, 101][..], Status::Failed),
        ] {
            let (status, detail) = evaluate(
                output(code, b"out\n", b"err\n"),
                OutcomePolicy::findings(codes),
            );
            assert_eq!(status, expected);
            assert_eq!(detail, "out\nerr\n");
        }
    }

    #[test]
    fn stdout_findings_only_apply_to_successful_processes() {
        let policy = OutcomePolicy {
            finding_codes: &[],
            stdout: StdoutPolicy::Findings,
        };
        assert_eq!(
            evaluate(output(0, b"main.go\n", b""), policy).0,
            Status::Failed
        );
        assert_eq!(evaluate(output(0, b"", b""), policy).0, Status::Passed);
        assert_eq!(
            evaluate(output(2, b"main.go\n", b""), policy).0,
            Status::Error
        );
    }

    #[test]
    fn launch_errors_and_silent_failures_remain_diagnosable() {
        let policy = OutcomePolicy::findings(&[1]);
        let (status, detail) = evaluate(Err(anyhow::anyhow!("cannot start")), policy);
        assert_eq!(status, Status::Error);
        assert!(detail.contains("cannot start"));
        let (status, detail) = evaluate(output(2, b"", b""), policy);
        assert_eq!(status, Status::Error);
        assert!(detail.contains("2"));
    }
}
