use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CommandCapture {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
    context: &str,
) -> Result<CommandCapture, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start {context}: {e}"))?;

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let (_, stderr) = read_child_output(&mut child);
                    let stderr = stderr.trim();
                    if stderr.is_empty() {
                        return Err(format!("{context} timed out after {}s", timeout.as_secs()));
                    }
                    return Err(format!(
                        "{context} timed out after {}s: {stderr}",
                        timeout.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("failed while waiting for {context}: {e}")),
        }
    };

    let (stdout, stderr) = read_child_output(&mut child);
    Ok(CommandCapture {
        status,
        stdout,
        stderr,
    })
}

fn read_child_output(child: &mut std::process::Child) -> (String, String) {
    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }

    (stdout, stderr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_command_with_timeout_captures_successful_output() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'ok'"]);

        let output =
            run_command_with_timeout(&mut command, Duration::from_secs(1), "shell command")
                .expect("command should succeed");

        assert!(output.status.success());
        assert_eq!(output.stdout, "ok");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn run_command_with_timeout_fails_fast_on_timeout() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 2"]);

        let err =
            run_command_with_timeout(&mut command, Duration::from_millis(200), "sleep command")
                .expect_err("command should time out");

        assert!(err.contains("timed out"));
    }
}
