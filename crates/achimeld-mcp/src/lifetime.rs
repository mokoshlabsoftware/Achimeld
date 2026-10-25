//! The stdio client must not keep the verified package alive after the editor closes.
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

/// Only the executable entry point uses this owner: returning ends the process.
/// Stdio may block forever (including writes when a client stops reading), so it
/// runs off the main thread and is deliberately not joined on editor shutdown.
/// The process releases its package locks and all threads when main returns.
pub(super) fn run(
    serve: impl FnOnce() -> Result<(), String> + Send + 'static,
    mut editor_available: impl FnMut() -> Result<bool, String>,
    absence_grace: Duration,
) -> Result<(), String> {
    let (finished, completion) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = finished.send(serve());
    });
    let mut absent_since = None;
    loop {
        if editor_available()? {
            absent_since = None;
        } else if absent_since.get_or_insert_with(Instant::now).elapsed() >= absence_grace {
            return Ok(());
        }
        // A short absence grace tolerates endpoint recreation between calls and
        // keeps a second open editor usable when the first one closes.
        match completion.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("MCP dispatcher stopped".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_last_editor_does_not_wait_for_idle_stdio() {
        let (release, blocked) = mpsc::channel::<()>();
        let started = Instant::now();
        let mut polls = 0;
        run(
            move || {
                let _ = blocked.recv();
                Ok(())
            },
            || {
                polls += 1;
                // One editor remains, a brief endpoint gap, then another editor
                // remains, then the final editor closes.
                Ok(matches!(polls, 1 | 3 | 4))
            },
            Duration::from_millis(150),
        )
        .unwrap();
        assert!(polls >= 7);
        assert!(started.elapsed() < Duration::from_secs(5));
        drop(release);
    }

    #[test]
    fn no_editor_at_startup_also_releases_idle_stdio() {
        let (release, blocked) = mpsc::channel::<()>();
        run(
            move || {
                let _ = blocked.recv();
                Ok(())
            },
            || Ok(false),
            Duration::from_millis(100),
        )
        .unwrap();
        drop(release);
    }

    #[test]
    fn client_exit_and_discovery_failure_do_not_leave_an_owner_waiting() {
        assert_eq!(
            run(
                || Err("client closed".into()),
                || Ok(true),
                Duration::from_secs(2)
            ),
            Err("client closed".into())
        );
        assert_eq!(
            run(
                || Ok(()),
                || Err("discovery failed".into()),
                Duration::from_secs(2)
            ),
            Err("discovery failed".into())
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Subprocess entry point for mcp_process_exits_with_open_stdin_after_last_editor_closes"]
    fn mcp_lifetime_child() {
        let fixture_pid: u32 = std::env::var("ACHIMELD_MCP_LIFETIME_FIXTURE")
            .unwrap()
            .parse()
            .unwrap();
        // Scope discovery to the fixture endpoint so an unrelated user editor
        // can stay open while this regression exercises real stdio shutdown.
        crate::run_with_discovery(|| {
            achimeld_security::editor_transport::instances()
                .map(|instances| {
                    instances.as_array().unwrap().iter().any(|instance| {
                        instance["processId"].as_u64() == Some(u64::from(fixture_pid))
                    })
                })
                .map_err(|error| error.to_string())
        })
        .unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Requires normal Windows named-pipe and process access"]
    fn mcp_process_exits_with_open_stdin_after_last_editor_closes() {
        for blocked_output in [false, true] {
            assert_process_exits(blocked_output);
        }
    }

    #[cfg(windows)]
    fn assert_process_exits(blocked_output: bool) {
        use achimeld_security::editor_transport::Pipe;
        use std::io::Write;
        use std::process::{Command, Stdio};
        let endpoint = Pipe::create().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "lifetime::tests::mcp_lifetime_child",
                "--ignored",
            ])
            .env(
                "ACHIMELD_MCP_LIFETIME_FIXTURE",
                std::process::id().to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(if blocked_output {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        if blocked_output {
            // tools/list exceeds the stdout pipe buffer. Do not consume it:
            // the facade must still exit while its response write is blocked.
            child.stdin.as_mut().unwrap().write_all(concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
            ).as_bytes()).unwrap();
        }
        // Keep stdin open and silent throughout, reproducing the original leak.
        std::thread::sleep(Duration::from_secs(3));
        let remained_alive = child.try_wait().unwrap().is_none();
        drop(endpoint);
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(
            remained_alive,
            "Connector exited while an editor was available"
        );
        assert!(
            status.is_some_and(|status| status.success()),
            "Idle connector survived editor shutdown"
        );
    }
}
