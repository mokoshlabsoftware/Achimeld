#![windows_subsystem = "windows"]
//! Console-free launch adapter for the unchanged .NET Playwright driver's stdio.
//! Only the packaged cli.js run-driver command and packaged Node runtime are allowed.
#[cfg(windows)]
fn run() -> Result<u32, Box<dyn std::error::Error>> {
    use windows_sys::Win32::System::Console::*;
    let mut session = achimeld_security::supervision::Session::connect()?;
    session.require_healthy()?;
    let root = session.package_root().to_path_buf();
    let cli = root.join(".playwright/package/cli.js");
    let node = root.join(".playwright/node/win32_x64/node.exe");
    achimeld_security::windows::reject_reparse(&cli)?;
    achimeld_security::windows::reject_reparse(&node)?;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 2
        || arguments[1] != "run-driver"
        || !achimeld_security::windows::same_path(
            &std::path::PathBuf::from(&arguments[0]).canonicalize()?,
            &cli.canonicalize()?,
        )
    {
        return Err("Only the packaged Playwright driver can run".into());
    }
    // SAFETY: GetStdHandle only borrows the process stdio handles; launch duplicates them before use.
    let streams = unsafe {
        [
            GetStdHandle(STD_INPUT_HANDLE),
            GetStdHandle(STD_OUTPUT_HANDLE),
            GetStdHandle(STD_ERROR_HANDLE),
        ]
    };
    launch(&node, &cli, &root, streams, || {
        session.require_healthy().map_err(Into::into)
    })
}
#[cfg(windows)]
fn launch(
    node: &std::path::Path,
    cli: &std::path::Path,
    root: &std::path::Path,
    streams: [windows_sys::Win32::Foundation::HANDLE; 3],
    mut health: impl FnMut() -> Result<(), Box<dyn std::error::Error>>,
) -> Result<u32, Box<dyn std::error::Error>> {
    use std::{
        ffi::c_void,
        ptr,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        System::{JobObjects::*, Threading::*},
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: Handle owns a successful Windows handle exclusively and closes it once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct Attributes(LPPROC_THREAD_ATTRIBUTE_LIST);
    impl Drop for Attributes {
        fn drop(&mut self) {
            // SAFETY: Attributes owns a successfully initialized list; backing storage still lives during Drop.
            unsafe {
                DeleteProcThreadAttributeList(self.0);
            }
        }
    }
    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().chain(Some(0)).collect()
    }
    let mut stdio = Vec::new();
    for stream in streams {
        let mut duplicate = ptr::null_mut();
        // Duplicate only the three Playwright streams; no supervision/file/Job handles.
        // SAFETY: Source handles are borrowed, outputs live, and each successful duplicate gets its own owner.
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                stream,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                1,
                DUPLICATE_SAME_ACCESS,
            )
        } == 0
        {
            return Err("Driver stream unavailable".into());
        }
        stdio.push(Handle(duplicate));
    }
    let handles: Vec<_> = stdio.iter().map(|handle| handle.0).collect();
    let mut size = 0;
    // SAFETY: The null-list size query uses live size storage and allocates no attribute list.
    unsafe {
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
    }
    if size == 0 {
        return Err("Restricted driver launch unavailable".into());
    }
    let mut storage = vec![0usize; size.div_ceil(size_of::<usize>())];
    let attributes = storage.as_mut_ptr().cast();
    // SAFETY: usize storage is aligned and sized from the API; it outlives the initialized attribute list.
    if unsafe { InitializeProcThreadAttributeList(attributes, 1, 0, &mut size) } == 0 {
        return Err("Restricted driver launch unavailable".into());
    }
    let attributes = Attributes(attributes);
    // SAFETY: Attribute and handle-list storage stay live through CreateProcessW; only owned stdio duplicates are listed.
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.0,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr().cast::<c_void>(),
            size_of_val(handles.as_slice()),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
        return Err("Driver handle restriction unavailable".into());
    }
    let startup = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: size_of::<STARTUPINFOEXW>() as u32,
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: handles[0],
            hStdOutput: handles[1],
            hStdError: handles[2],
            ..Default::default()
        },
        lpAttributeList: attributes.0,
    };
    // SAFETY: Null security/name pointers request an unnamed non-inheritable Job; the result is checked.
    let job = Handle(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
    if job.0.is_null() {
        return Err("Driver lifetime unavailable".into());
    }
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags =
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    limits.BasicLimitInformation.ActiveProcessLimit = 256;
    // SAFETY: Job is owned and live; the initialized limits struct has the supplied size.
    if unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err("Driver lifetime unavailable".into());
    }
    let path = wide(node.as_os_str());
    let mut command = achimeld_security::windows_arguments::command_line(
        node.as_os_str(),
        &[cli.as_os_str(), std::ffi::OsStr::new("run-driver")],
    )?;
    let directory = wide(root.as_os_str());
    let mut info = PROCESS_INFORMATION::default();
    // Explicit handle list, inherited outer Jobs, and assignment before execution.
    // SAFETY: UTF-16/environment buffers, stdio owners and attributes stay live; child is suspended until Job assignment.
    if unsafe {
        CreateProcessW(
            path.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_SUSPENDED | DETACHED_PROCESS | EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            directory.as_ptr(),
            &startup.StartupInfo,
            &mut info,
        )
    } == 0
    {
        return Err("Packaged driver could not start".into());
    }
    let process = Handle(info.hProcess);
    let thread = Handle(info.hThread);
    // SAFETY: Both handles are owned and live; assignment happens before the suspended child resumes.
    if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
        // SAFETY: Owned process handle remains live during termination and the bounded wait.
        unsafe {
            TerminateProcess(process.0, 125);
            WaitForSingleObject(process.0, 5000);
        }
        return Err("Driver Job assignment failed".into());
    }
    // SAFETY: Thread is owned and the child has already joined its lifetime Job.
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        return Err("Driver could not resume".into());
    }
    drop(stdio);
    drop(attributes);
    drop(thread);
    let mut last_health = Instant::now();
    loop {
        // SAFETY: Process is owned and live throughout this bounded wait.
        match unsafe { WaitForSingleObject(process.0, 100) } {
            WAIT_OBJECT_0 => {
                let mut code = 125;
                // SAFETY: Process handle and exit-code output remain live during the query.
                if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 {
                    return Err("Driver exit status unavailable".into());
                }
                return Ok(code);
            }
            WAIT_TIMEOUT => {}
            _ => return Err("Driver lifetime check failed".into()),
        }
        if last_health.elapsed() >= Duration::from_secs(1) {
            health()?;
            last_health = Instant::now();
        }
    }
}
#[cfg(all(test, windows))]
mod tests {
    use std::{
        fs::File,
        io::{Read, Write},
        os::windows::io::{AsRawHandle, FromRawHandle},
        ptr,
        time::{Duration, Instant},
    };
    fn pipe() -> (File, File) {
        let mut read = ptr::null_mut();
        let mut write = ptr::null_mut();
        assert_ne!(
            // SAFETY: Test output pointers are live; successful pipe creation gives two distinct owned handles.
            unsafe {
                windows_sys::Win32::System::Pipes::CreatePipe(
                    &mut read,
                    &mut write,
                    ptr::null(),
                    65536,
                )
            },
            0
        );
        // SAFETY: Successful CreatePipe handles transfer exactly once into their corresponding File owners.
        unsafe { (File::from_raw_handle(read), File::from_raw_handle(write)) }
    }
    #[test]
    #[ignore = "Requires built C# fixture; build/Test-Bridge.ps1 runs this gate"]
    fn stdio_driver_round_trip_and_health_loss_terminate_child() {
        let node = std::path::PathBuf::from(std::env::var_os("ACHIMELD_TEST_WORKER").unwrap());
        for hang in [false, true] {
            let (input, mut writer) = pipe();
            let (mut reader, output) = pipe();
            let (mut errors, error) = pipe();
            let streams = [
                input.as_raw_handle(),
                output.as_raw_handle(),
                error.as_raw_handle(),
            ];
            let bytes = "Żółw🦀".repeat(20000).into_bytes();
            let expected = bytes.clone();
            let peer = std::thread::spawn(move || {
                if hang {
                    let mut value = Vec::new();
                    reader.read_to_end(&mut value).unwrap();
                    return value;
                }
                writer
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .unwrap();
                writer.write_all(&bytes).unwrap();
                drop(writer);
                let mut header = [0; 4];
                reader.read_exact(&mut header).unwrap();
                let mut response = vec![0; u32::from_le_bytes(header) as usize];
                reader.read_exact(&mut response).unwrap();
                response
            });
            let started = Instant::now();
            let result = super::launch(
                &node,
                std::path::Path::new(if hang { "hang.js" } else { "echo.js" }),
                node.parent().unwrap(),
                streams,
                || {
                    if hang {
                        Err("supervisor lost".into())
                    } else {
                        Ok(())
                    }
                },
            );
            drop(input);
            drop(output);
            drop(error);
            let response = peer.join().unwrap();
            let mut error_text = Vec::new();
            errors.read_to_end(&mut error_text).unwrap();
            if hang {
                assert!(result.is_err());
                assert!(started.elapsed() < Duration::from_secs(3));
                assert!(response.is_empty());
            } else {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(response, expected);
                assert_eq!(error_text, b"fixture stderr");
            }
        }
    }

    #[test]
    #[ignore = "Requires the patched Playwright worker; build/Test-BrowserRequests.ps1 runs this gate"]
    fn stdio_driver_playwright_forced_cleanup_and_health_loss_terminate_descendants() {
        use std::{
            io::{BufRead, BufReader},
            os::windows::io::OwnedHandle,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };
        use windows_sys::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::*};

        let root =
            std::path::PathBuf::from(std::env::var_os("ACHIMELD_TEST_PLAYWRIGHT_WORKER").unwrap());
        let node = root.join(".playwright/node/win32_x64/node.exe");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/playwright-runtime-tests/cleanup.js");
        for health_loss in [false, true] {
            let (input, mut writer) = pipe();
            let (reader, output) = pipe();
            let (mut errors, error) = pipe();
            let streams = [
                input.as_raw_handle(),
                output.as_raw_handle(),
                error.as_raw_handle(),
            ];
            let ready = Arc::new(AtomicBool::new(false));
            let peer_ready = ready.clone();
            let peer = std::thread::spawn(move || {
                let mut reader = BufReader::new(reader);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let processes: Vec<_> = line
                    .split_whitespace()
                    .map(|pid| {
                        // SAFETY: Fixture PID is only used to obtain a synchronization handle; check before ownership transfer.
                        let handle =
                            unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid.parse().unwrap()) };
                        assert!(!handle.is_null());
                        // SAFETY: Each successful OpenProcess handle transfers once to its exclusive owner.
                        unsafe { OwnedHandle::from_raw_handle(handle) }
                    })
                    .collect();
                assert_eq!(
                    processes.len(),
                    2,
                    "Fixture must expose parent and descendant"
                );
                writer
                    .write_all(if health_loss { b"health\n" } else { b"force\n" })
                    .unwrap();
                peer_ready.store(true, Ordering::Release);
                let mut response = String::new();
                reader.read_to_string(&mut response).unwrap();
                (processes, response)
            });
            let started = Instant::now();
            let result = super::launch(&node, &fixture, &root, streams, || {
                if health_loss && ready.load(Ordering::Acquire) {
                    Err("supervisor lost".into())
                } else if started.elapsed() > Duration::from_secs(20) {
                    Err("Playwright cleanup fixture timed out".into())
                } else {
                    Ok(())
                }
            });
            drop(input);
            drop(output);
            drop(error);
            let (processes, response) = peer.join().unwrap();
            let mut stderr = String::new();
            errors.read_to_string(&mut stderr).unwrap();
            assert!(stderr.is_empty(), "{stderr}");
            if health_loss {
                assert!(result.is_err());
                assert!(response.is_empty());
            } else {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(response, "cleanup-direct\n");
            }
            for process in processes {
                assert_eq!(
                    // SAFETY: Each synchronization handle remains owned for this bounded exit wait.
                    unsafe { WaitForSingleObject(process.as_raw_handle(), 5000) },
                    WAIT_OBJECT_0,
                    "Node host Job must terminate every browser descendant"
                );
            }
        }
    }
}
fn main() {
    #[cfg(windows)]
    let code = run().unwrap_or(125);
    #[cfg(not(windows))]
    let code = 125;
    std::process::exit(code as i32);
}
