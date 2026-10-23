#[test]
fn bootstrapper_identity_preserves_shared_vectors_and_invalid_utf16() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::Path};
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/security/path-identity.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        assert_eq!(
            super::same_image(
                Path::new(case["a"].as_str().unwrap()),
                Path::new(case["b"].as_str().unwrap())
            ),
            case["equal"].as_bool().unwrap()
        );
    }
    let mut first: Vec<u16> = "C:\\package\\".encode_utf16().collect();
    first.push(0xd800);
    let mut other = first.clone();
    *other.last_mut().unwrap() = 0xd801;
    let first = std::path::PathBuf::from(OsString::from_wide(&first));
    let other = std::path::PathBuf::from(OsString::from_wide(&other));
    assert!(super::same_image(&first, &first));
    assert!(!super::same_image(&first, &other));
}
#[test]
fn executable_identity_accepts_windows_separators_but_rejects_other_paths() {
    use std::path::Path;
    let expected = Path::new(
        "C:\\Program Files\\Achimeld-Development\\version\\.playwright/node/win32_x64/node.exe",
    );
    assert!(super::same_image(
        expected,
        Path::new(
            "c:\\program files\\achimeld-development\\version\\.playwright\\node\\win32_x64\\NODE.EXE"
        )
    ));
    for path in [
        "C:\\Program Files\\Achimeld-Development\\other\\.playwright\\node\\win32_x64\\node.exe",
        "C:\\Users\\someone\\node.exe",
        "C:\\Program Files\\Achimeld-Development\\version\\.playwright\\node\\win32_x64\\node.exe.evil",
        ".playwright/node/win32_x64/node.exe",
    ] {
        assert!(!super::same_image(expected, Path::new(path)));
    }
}
#[test]
#[ignore = "Requires the built C# fixture; build/Test-Bridge.ps1 runs this gate"]
fn console_app_launch_does_not_add_system_children() {
    use std::time::Duration;
    let fixture = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_TEST_WORKER").expect("Build the C# fixture first"),
    );
    let root = std::env::temp_dir().join(format!(
        "achimeld-console-test-{:016x}",
        rand::random::<u64>()
    ));
    std::fs::create_dir(&root).unwrap();
    for file in std::fs::read_dir(fixture.parent().unwrap()).unwrap() {
        let file = file.unwrap();
        if file.file_type().unwrap().is_file() {
            std::fs::copy(file.path(), root.join(file.file_name())).unwrap();
        }
    }
    std::fs::copy(&fixture, root.join("Achimeld.App.exe")).unwrap();
    let job = super::Job::create().unwrap();
    let child = job
        .launch(
            &root,
            "unused-test-endpoint",
            &[std::ffi::OsStr::new("--child")],
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(250));
    let mut code = 0;
    // SAFETY: Test child handle is owned and the exit-code output remains live for the query.
    unsafe {
        windows_sys::Win32::System::Threading::GetExitCodeProcess(child.raw(), &mut code);
    }
    assert_eq!(code, windows_sys::Win32::Foundation::STILL_ACTIVE as u32);
    let pids = job.pids().unwrap();
    let images: Vec<_> = pids.iter().map(|pid| super::image(*pid).unwrap()).collect();
    let inspected = super::monitored_image(pids[0]).unwrap().unwrap();
    assert_eq!(inspected.image(), images[0]);
    assert!(!inspected.has_exited().unwrap());
    assert_eq!(
        pids.len(),
        1,
        "Console host or unexpected helper entered the Job: {images:?}"
    );
    assert!(
        super::image(pids[0])
            .unwrap()
            .to_string_lossy()
            .eq_ignore_ascii_case(&root.join("Achimeld.App.exe").to_string_lossy())
    );
    // A synchronize-only handle deliberately denies image queries. If teardown
    // completes during the bounded grace period, the same handle proves exit.
    // SAFETY: Test PID is live and a successful handle transfers once to Handle.
    let raw = unsafe {
        windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE,
            0,
            pids[0],
        )
    };
    assert!(!raw.is_null());
    let synchronize_only = super::handles::Handle(raw as usize);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(50));
            job.terminate();
        });
        assert_eq!(
            super::identity::image_if_live(&synchronize_only).unwrap(),
            None
        );
    });
    assert_eq!(
        // SAFETY: Test child handle is still owned while waiting for termination.
        unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(child.raw(), 5000) },
        windows_sys::Win32::Foundation::WAIT_OBJECT_0
    );
    // The retained child handle keeps the exited process object alive. Its
    // Job membership may outlive the image name; the signaled handle proves exit.
    assert!(inspected.has_exited().unwrap());
    assert!(super::monitored_image(pids[0]).unwrap().is_none());
    drop(inspected);
    drop(child);
    drop(job);
    // Exercise the complete validated CLI-to-CreateProcess path, not just quoting.
    let read_job = super::Job::create().unwrap();
    let arguments = crate::commands::read_argument(&["--read-steam".into(), "480".into()])
        .unwrap()
        .unwrap();
    let arguments: Vec<&std::ffi::OsStr> = arguments.iter().map(std::ffi::OsStr::new).collect();
    let child = read_job
        .launch(&root, "unused-test-endpoint", &arguments)
        .unwrap();
    // SAFETY: The fixture process handle and initialized exit-code storage remain live.
    unsafe {
        assert_eq!(
            windows_sys::Win32::System::Threading::WaitForSingleObject(child.raw(), 5000),
            windows_sys::Win32::Foundation::WAIT_OBJECT_0
        );
        assert_ne!(
            windows_sys::Win32::System::Threading::GetExitCodeProcess(child.raw(), &mut code),
            0
        );
    }
    assert_eq!(
        code, 0,
        "Validated read command must arrive as two arguments"
    );
    drop(child);
    drop(read_job);
    let actual = root.canonicalize().unwrap();
    assert_eq!(
        actual.parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    assert!(
        actual
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("achimeld-console-test-")
    );
    std::fs::remove_dir_all(actual).unwrap();
}
#[test]
fn nonblocking_pipe_delivers_maximum_response_to_a_delayed_reader() {
    use std::{
        fs::OpenOptions,
        io::{Read, Write},
        time::{Duration, Instant},
    };
    let name = format!("Achimeld.Security.Test.{:016x}", rand::random::<u64>());
    let pipe = super::Pipe::create(&name).unwrap();
    let client = std::thread::spawn(move || {
        let mut client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("\\\\.\\pipe\\{name}"))
            .unwrap();
        let request = b"null";
        client
            .write_all(&(request.len() as u32).to_le_bytes())
            .unwrap();
        client.write_all(request).unwrap();
        // The framing and maximum-size body exceed the pipe's output buffer.
        std::thread::sleep(Duration::from_millis(100));
        let mut header = [0; 4];
        client.read_exact(&mut header).unwrap();
        assert_eq!(u32::from_le_bytes(header), 65536);
        let mut body = vec![0; 65536];
        client.read_exact(&mut body).unwrap();
        assert_eq!(
            serde_json::from_slice::<String>(&body).unwrap(),
            "x".repeat(65534)
        );
        client.write_all(b"ACK1").unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while pipe.client().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let result = pipe.exchange(|_| Ok(serde_json::Value::String("x".repeat(65534))));
    pipe.disconnect();
    let client_result = client.join();
    result.unwrap();
    client_result.unwrap();
}
#[test]
fn nonblocking_pipe_waits_for_request_and_response_receipt() {
    use std::{
        fs::OpenOptions,
        io::{Read, Write},
        time::{Duration, Instant},
    };
    let name = format!("Achimeld.Security.Test.{:016x}", rand::random::<u64>());
    let pipe = super::Pipe::create(&name).unwrap();
    let client = std::thread::spawn(move || {
        let mut client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("\\\\.\\pipe\\{name}"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let request = br#"{"operation":"test"}"#;
        client
            .write_all(&(request.len() as u32).to_le_bytes())
            .unwrap();
        client.write_all(request).unwrap();
        let mut header = [0; 4];
        client.read_exact(&mut header).unwrap();
        let mut body = vec![0; u32::from_le_bytes(header) as usize];
        client.read_exact(&mut body).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["operation"],
            "test"
        );
        client.write_all(b"ACK1").unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while pipe.client().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    pipe.exchange(Ok).unwrap();
    pipe.disconnect();
    client.join().unwrap();
}
#[test]
fn installation_access_check_rejects_a_user_writable_directory() {
    let fixture =
        std::env::temp_dir().join(format!("achimeld-acl-test-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&fixture).unwrap();
    let result = super::require_protected_package(&fixture);
    std::fs::remove_dir(&fixture).unwrap();
    // Distinguish a real effective-write denial from an invalid descriptor/API error.
    assert!(
        result
            .unwrap_err()
            .starts_with("Installation is writable by the app user.")
    );
}
#[test]
fn abandoned_pipe_connection_does_not_poison_the_next_peer() {
    use std::fs::OpenOptions;
    let name = format!("Achimeld.Security.Test.{:016x}", rand::random::<u64>());
    let pipe = super::Pipe::create(&name).unwrap();
    assert!(pipe.client().unwrap().is_none());
    assert!(pipe.client().unwrap().is_none());
    let open = || {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("\\\\.\\pipe\\{name}"))
            .unwrap()
    };
    drop(open());
    assert!(pipe.client().unwrap().is_none());
    // A disconnected PIPE_NOWAIT instance becomes available on the next poll.
    assert!(pipe.client().unwrap().is_none());
    let next = open();
    let peer = pipe.client().unwrap().or_else(|| pipe.client().unwrap());
    assert_eq!(peer, Some(std::process::id()));
    pipe.disconnect();
    drop(next);
}

#[test]
fn path_identity_conforms_to_shared_vectors_and_preserves_invalid_utf16() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::Path};
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/security/path-identity.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        assert_eq!(
            super::same_image(
                Path::new(case["a"].as_str().unwrap()),
                Path::new(case["b"].as_str().unwrap())
            ),
            case["equal"].as_bool().unwrap()
        );
    }
    let prefix: Vec<_> = "C:/Users/".encode_utf16().collect();
    let path = |unit| {
        let mut units = prefix.clone();
        units.push(unit);
        units.extend("/Achimeld.exe".encode_utf16());
        OsString::from_wide(&units)
    };
    assert!(!super::same_image(
        Path::new(&path(0xd800)),
        Path::new(&path(0xd801))
    ));
    assert!(!super::same_image(
        Path::new(&path(0xd800)),
        Path::new(&path(0xfffd))
    ));
}
