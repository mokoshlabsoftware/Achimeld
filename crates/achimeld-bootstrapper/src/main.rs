#[cfg(windows)]
mod broker;
mod commands;
mod manifest;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod splash;
#[cfg(windows)]
mod supervision;
#[cfg(windows)]
mod telemetry;
#[cfg(windows)]
mod udp_probes;
#[cfg(windows)]
mod wfp;

#[cfg(windows)]
fn run(splash: Option<&splash::Splash>) -> Result<(), String> {
    use rand::RngCore;
    use std::{
        collections::{HashMap, HashSet, VecDeque},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    let executable = std::env::current_exe().map_err(|_| "bootstrapper identity unavailable")?;
    let root = executable
        .parent()
        .ok_or("installation path unavailable")?
        .to_owned();
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|arg| arg == "--verify-package")
        && arguments.len() == 2
    {
        let package = manifest::verify(std::path::Path::new(&arguments[1]))?;
        println!(
            "Verified {} {} package",
            package.manifest().mode,
            package.manifest().version
        );
        return Ok(());
    }
    if arguments
        .first()
        .is_some_and(|arg| arg == "--check-package-policy")
        && arguments.len() == 2
    {
        let package = manifest::verify(std::path::Path::new(&arguments[1]))?;
        wfp::Engine::open()?.verify(&package)?;
        println!("Persistent Windows restrictions verified");
        return Ok(());
    }
    let (protected, arguments) = commands::session_arguments(&arguments)?;
    let package = Arc::new(manifest::verify(&root)?);
    if arguments == ["--mcp"] {
        if platform::elevated()? {
            return Err("Start the MCP server as a normal user".into());
        }
        return achimeld_mcp::run();
    }
    let read_argument = commands::read_argument(arguments)?;
    let self_test = matches!(
        arguments.first().map(String::as_str),
        Some("--self-test" | "--self-test-hold")
    ) && arguments.len() == 1;
    let hold_test = arguments == ["--self-test-hold"];
    let user_self_test = arguments == ["--user-self-test"];
    if arguments == ["--hold-verification"] && package.manifest().mode == "development" {
        println!("Verification locks active");
        use std::io::Write;
        std::io::stdout()
            .flush()
            .map_err(|_| "verification output unavailable")?;
        std::thread::sleep(Duration::from_secs(10));
        return Ok(());
    }
    if arguments == ["--verify"] {
        println!(
            "Verified {} {} package",
            package.manifest().mode,
            package.manifest().version
        );
        return Ok(());
    }
    match arguments.first().map(String::as_str) {
        Some("--verify") if arguments.len() == 1 => {
            println!(
                "Verified {} {} package",
                package.manifest().mode,
                package.manifest().version
            );
            return Ok(());
        }
        Some("--install-policy") if arguments.len() == 1 => {
            if !platform::elevated()? {
                return Err("policy installation requires Windows administrator approval".into());
            }
            wfp::Engine::open()?.install(&package)?;
            println!("Persistent Windows restrictions installed and verified");
            return Ok(());
        }
        Some("--remove-policy") if arguments.len() == 1 => {
            if !platform::elevated()? {
                return Err("policy removal requires Windows administrator approval".into());
            }
            wfp::Engine::open()?.remove(&package)?;
            println!("Package-specific restrictions removed");
            return Ok(());
        }
        Some("--check-policy") if arguments.len() == 1 => {
            wfp::Engine::open()?.verify(&package)?;
            println!("Persistent Windows restrictions verified");
            return Ok(());
        }
        Some("--self-test" | "--self-test-hold")
            if arguments.len() == 1 && package.manifest().mode == "development" => {}
        Some("--user-self-test")
            if arguments.len() == 1 && package.manifest().mode == "development" => {}
        Some("--read-steam") if read_argument.is_some() => {}
        Some(_) => return Err("unsupported bootstrapper command".into()),
        None => {}
    }
    if platform::elevated()? {
        return Err(
            "Start Achimeld as a normal user. Only policy installation may run as administrator."
                .into(),
        );
    }
    // The requested protection level is fixed before launching any application
    // code. User sessions retain inventory locks, Jobs, IPC and broker policy;
    // only explicit protected sessions claim administrator-installed isolation.
    let network_enforced = protected || self_test;
    if network_enforced {
        platform::require_protected_package(&root)?;
        wfp::Engine::open()?.verify(&package)?;
    }
    let mut udp_probes = if network_enforced {
        Some(udp_probes::Probes::create()?)
    } else {
        None
    };
    let _probes = if self_test {
        Some((
            std::net::TcpListener::bind(("127.0.0.1", 42762))
                .map_err(|_| "IPv4 test endpoint unavailable")?,
            std::net::TcpListener::bind(("::1", 42762))
                .map_err(|_| "IPv6 test endpoint unavailable")?,
        ))
    } else {
        None
    };
    let profile = platform::profile()?;
    let job = Arc::new(platform::Job::create()?);
    // Test all four collectors before allowing any application code to execute.
    telemetry::collect(&HashSet::new())?;
    let healthy = Arc::new(AtomicBool::new(true));
    let observations = Arc::new(Mutex::new(VecDeque::<telemetry::Observation>::new()));
    let mut random = [0; 32];
    rand::thread_rng().fill_bytes(&mut random);
    let session = hex::encode(random);
    let pipe_name = format!("Achimeld.Security.{}", session);
    let pipe = platform::Pipe::create(&pipe_name)?;
    let broker = broker::start(package.clone(), job.clone(), healthy.clone())?;
    let monitor_package = package.clone();
    let monitor_job = job.clone();
    let monitor_healthy = healthy.clone();
    let monitor_observations = observations.clone();
    let monitor = std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let engine = if network_enforced {
                Some(wfp::Engine::open()?)
            } else {
                None
            };
            let mut inspected = HashMap::<u32, platform::MonitoredProcess>::new();
            while monitor_healthy.load(Ordering::Acquire) {
                if let Some(engine) = &engine {
                    engine.verify(&monitor_package)?;
                }
                let pids = monitor_job.pids()?;
                inspected.retain(|pid, _| pids.contains(pid));
                for pid in &pids {
                    if let Some(process) = inspected.get(pid) {
                        if !process.has_exited()? {
                            // Image identity cannot change for this live process
                            // object. Keep its handle, rather than querying again
                            // while Chromium unmaps its image during teardown.
                            continue;
                        }
                        // Never let an exited handle authenticate a reused PID.
                        inspected.remove(pid);
                    }
                    let process = match platform::monitored_image(*pid) {
                        Ok(Some(process)) => process,
                        Ok(None) => continue,
                        // Re-enumerate the Job on open/query failure. An access-denied
                        // process handle is not proof that a member has departed.
                        Err(_) if !monitor_job.pids()?.contains(pid) => continue,
                        Err(error) => return Err(format!("{error} (PID {pid})")),
                    };
                    if !monitor_package.manifest().network.programs.iter().any(|p| {
                        platform::same_image(&monitor_package.root().join(&p.path), process.image())
                    }) {
                        return Err(format!("unapproved child process detected (PID {pid})"));
                    }
                    inspected.insert(*pid, process);
                }
                let mut pids: HashSet<_> = pids.into_iter().collect();
                pids.insert(std::process::id());
                let records = telemetry::collect(&pids)?;
                let mut log = monitor_observations
                    .lock()
                    .map_err(|_| "security report unavailable")?;
                for record in records {
                    if log.len() >= 256 {
                        log.pop_front();
                    }
                    log.push_back(record);
                }
                drop(log);
                std::thread::sleep(Duration::from_secs(1));
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Security monitoring stopped: {error}");
            monitor_healthy.store(false, Ordering::Release);
            monitor_job.terminate();
        }
    });
    let editor_arguments: Vec<String> = if user_self_test {
        vec!["--security-user-self-test".into()]
    } else if hold_test {
        vec!["--security-self-test-hold".into()]
    } else if self_test {
        vec!["--security-self-test".into()]
    } else {
        read_argument.into_iter().flatten().collect()
    };
    let editor_arguments: Vec<&std::ffi::OsStr> =
        editor_arguments.iter().map(std::ffi::OsStr::new).collect();
    let editor = job.launch(&root, &pipe_name, &editor_arguments)?;
    if let Some(splash) = splash {
        // The splash belongs to the bootstrapper, never to the supervised Job.
        // SAFETY: The editor process handle remains owned and live through this query.
        splash.watch_editor(unsafe {
            windows_sys::Win32::System::Threading::GetProcessId(editor.raw())
        });
        desktop_status(
            "Monitoring Achimeld while it runs.\n\
             You can minimize this window. Closing it will also stop Achimeld.\n\
             The bootstrapper exits automatically when you close the main app.",
        );
    }
    let mut failure = false;
    let mut rejected_connections = 0;
    // SAFETY: editor is an owned live process handle for the entire bounded wait loop.
    while unsafe { WaitForSingleObject(editor.raw(), 10) } == 258 {
        if !healthy.load(Ordering::Acquire) || broker.is_finished() || monitor.is_finished() {
            failure = true;
            job.terminate();
            break;
        }
        let peer = match pipe.client() {
            Ok(peer) => peer,
            Err(_) => {
                healthy.store(false, Ordering::Release);
                job.terminate();
                failure = true;
                break;
            }
        };
        if let Some(pid) = peer {
            let mut dispatcher = supervision::Dispatcher {
                package: &package,
                job: &job,
                profile: &profile,
                session: &session,
                supervisor_pid: std::process::id(),
                healthy: &healthy,
                network_enforced,
                observations: &observations,
                probes: udp_probes.as_mut(),
            };
            let result = dispatcher
                .authorize_peer(pid)
                .map_err(|error| error.code().to_owned())
                .and_then(|_| {
                    pipe.exchange(|value| match dispatcher.respond(pid, value) {
                        Ok(status) => serde_json::to_value(status)
                            .map_err(|_| "response-serialization".into()),
                        Err(error) => {
                            if matches!(error, supervision::DispatchError::ControlLost(_)) {
                                healthy.store(false, Ordering::Release);
                                job.terminate();
                            }
                            Err(error.code().to_owned())
                        }
                    })
                });
            if result.is_err() && rejected_connections < 16 {
                // SECURITY(SEC-DIAGNOSTICS-001): never print pipe content or caller-provided errors.
                eprintln!("Supervision connection rejected");
                rejected_connections += 1;
            }
            pipe.disconnect();
        }
    }
    let mut exit_code = 125;
    // SAFETY: editor handle and exit-code output remain live during this synchronous query.
    unsafe {
        windows_sys::Win32::System::Threading::GetExitCodeProcess(editor.raw(), &mut exit_code);
    }
    let security_failed = failure || !healthy.load(Ordering::Acquire);
    healthy.store(false, Ordering::Release);
    job.terminate();
    let _ = broker.join();
    let _ = monitor.join();
    if self_test {
        if exit_code != 0 || failure {
            return Err(format!(
                "Installed-package network self-test failed (child exit {exit_code}, supervision failure {failure})"
            ));
        }
        println!(
            "Installed-package IPv4/IPv6 TCP/UDP denial and permitted Steam broker tests passed"
        );
    }
    if security_failed {
        Err("Mandatory security supervision failed. Automation was stopped; reconcile any interrupted Steam changes before retrying.".into())
    } else if exit_code != 0 {
        Err("The Achimeld editor stopped unexpectedly. Check the redacted startup diagnostics before retrying.".into())
    } else {
        if user_self_test {
            println!(
                "User-package supervision and worker handshake passed. No browser or live Steam access performed; Windows network isolation is not enabled."
            );
        }
        Ok(())
    }
}
#[cfg(not(windows))]
fn run() -> Result<(), String> {
    Err("Achimeld requires Windows".into())
}
#[cfg(windows)]
fn desktop_status(message: &str) {
    use std::io::Write;
    // Status output is best-effort: a missing or closed terminal must not panic
    // the supervisor. CLI/MCP output never passes through this desktop-only path.
    let mut output = std::io::stdout().lock();
    let _ = writeln!(output, "{message}");
    let _ = output.flush();
}
fn main() {
    #[cfg(windows)]
    {
        let arguments: Vec<_> = std::env::args().skip(1).collect();
        let desktop = arguments.is_empty() || arguments == ["--protected"];
        if desktop {
            // Keep the console subsystem for the installer and CLI tools. Name
            // visible terminals before attempting to hide an owned legacy console;
            // GetConsoleWindow does not identify the visible Windows Terminal UI.
            let title: Vec<u16> = "Achimeld Bootstrapper\0".encode_utf16().collect();
            // SAFETY: The title is NUL-terminated and live for SetConsoleTitleW.
            // The other calls inspect this process's console. Hide it only when
            // this process is its sole client, so a terminal that launched Achimeld
            // remains visible to its user.
            unsafe {
                windows_sys::Win32::System::Console::SetConsoleTitleW(title.as_ptr());
                let console = windows_sys::Win32::System::Console::GetConsoleWindow();
                let mut clients = [0_u32; 2];
                let count = windows_sys::Win32::System::Console::GetConsoleProcessList(
                    clients.as_mut_ptr(),
                    clients.len() as u32,
                );
                if !console.is_null() && count == 1 && clients[0] == std::process::id() {
                    windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                        console,
                        windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
                    );
                }
            }
            desktop_status(
                "Achimeld Bootstrapper\n\
                 =====================\n\
                 This companion process starts and monitors the main Achimeld app.\n\n\
                 Checking application files and preparing the supervised session...",
            );
        }
        let splash = desktop.then(splash::Splash::start);
        if let Err(error) = run(splash.as_ref()) {
            eprintln!("Achimeld could not start: {error}");
            if let Some(splash) = splash {
                splash.show_failure();
            }
            std::process::exit(125);
        }
        if desktop {
            desktop_status(
                "Achimeld has closed. Monitoring finished; the bootstrapper is exiting.",
            );
        }
    }
    #[cfg(not(windows))]
    if let Err(error) = run() {
        eprintln!("Achimeld could not start: {error}");
        std::process::exit(125);
    }
}
