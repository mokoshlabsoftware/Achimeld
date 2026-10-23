//! Authenticated loopback tunnel lifecycle; destination policy is in policy.rs.
use super::policy::{connect_host, public_ip};
use crate::{
    manifest::{ProgramRole, VerifiedPackage},
    platform::{self, Job},
    telemetry,
};
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    os::windows::io::FromRawSocket,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::Networking::WinSock::*;

pub fn start(
    package: Arc<VerifiedPackage>,
    job: Arc<Job>,
    healthy: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    let port = package.manifest().network.proxy_port;
    let listener = exclusive_listener(port)?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "broker listener unavailable")?;
    let active = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));
    let connected = Arc::new(AtomicBool::new(false));
    Ok(std::thread::spawn(move || {
        while healthy.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((client, peer)) => {
                    if !peer.ip().is_loopback() || active.fetch_add(1, Ordering::AcqRel) >= 64 {
                        active.fetch_sub(1, Ordering::AcqRel);
                        continue;
                    }
                    let package = package.clone();
                    let job = job.clone();
                    let healthy = healthy.clone();
                    let active = active.clone();
                    let failures = failures.clone();
                    let connected = connected.clone();
                    std::thread::spawn(move || {
                        if let Err(error) = serve(
                            client,
                            peer.port(),
                            port,
                            &package,
                            &job,
                            &healthy,
                            &connected,
                        ) {
                            if package.manifest().mode == "development"
                                && failures.fetch_add(1, Ordering::Relaxed) < 16
                            {
                                // Fixed stages only: never log the request, host, payload or raw OS error.
                                eprintln!(
                                    "Steam broker stopped a connection at {}",
                                    diagnostic_stage(&error)
                                );
                            }
                        }
                        active.fetch_sub(1, Ordering::AcqRel);
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => {
                    healthy.store(false, Ordering::Release);
                    job.terminate();
                    break;
                }
            }
        }
    }))
}
fn diagnostic_stage(error: &str) -> &'static str {
    match error {
        "broker client attribution unavailable" => "client-attribution",
        "broker client is not supervised" => "client-supervision",
        "broker client role rejected" => "client-role",
        "destination is not approved" => "destination-denied",
        "approved destination DNS failed" => "approved-dns",
        "nonpublic destination rejected" => "destination-address",
        "approved destination unavailable" => "approved-connect",
        "supervision unavailable" => "session-health",
        "broker request unavailable" => "request-read",
        "broker request interrupted" => "request-eof",
        "broker request deadline exceeded" => "request-deadline",
        "unapproved broker request header" => "request-header",
        "broker request host mismatch" => "request-host",
        "invalid broker request header" => "request-header-format",
        "invalid broker request framing" => "request-framing",
        "broker client disconnected" => "client-response",
        "upstream tunnel transfer failed" | "client tunnel transfer failed" => "tunnel-transfer",
        _ => "connection-validation",
    }
}
fn configure_client(client: &TcpStream) -> Result<(), String> {
    // Winsock accepts inherit the listener's nonblocking mode. Dedicated tunnel
    // threads use bounded blocking reads/writes; the listener stays nonblocking.
    client
        .set_nonblocking(false)
        .map_err(|_| "broker socket mode unavailable")?;
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| "broker timeout unavailable")?;
    client
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| "broker timeout unavailable")?;
    Ok(())
}
fn exclusive_listener(port: u16) -> Result<TcpListener, String> {
    // SAFETY: Winsock outputs/options and address structs remain live; successful socket transfers once to TcpListener before fallible configuration.
    unsafe {
        let mut data = WSADATA::default();
        if WSAStartup(0x0202, &mut data) != 0 {
            return Err("Windows socket subsystem unavailable".into());
        }
        let raw = socket(AF_INET as i32, SOCK_STREAM, IPPROTO_TCP);
        if raw == INVALID_SOCKET {
            return Err("broker socket unavailable".into());
        }
        let listener = TcpListener::from_raw_socket(raw as u64);
        let exclusive = 1_i32;
        if setsockopt(
            raw,
            SOL_SOCKET,
            SO_EXCLUSIVEADDRUSE,
            &exclusive as *const _ as *const u8,
            size_of::<i32>() as i32,
        ) != 0
        {
            return Err("exclusive broker endpoint unavailable".into());
        }
        let address = SOCKADDR_IN {
            sin_family: AF_INET,
            sin_port: port.to_be(),
            sin_addr: IN_ADDR {
                S_un: IN_ADDR_0 {
                    S_addr: u32::from_ne_bytes([127, 0, 0, 1]),
                },
            },
            ..Default::default()
        };
        if bind(
            raw,
            &address as *const _ as *const SOCKADDR,
            size_of::<SOCKADDR_IN>() as i32,
        ) != 0
            || listen(raw, 64) != 0
        {
            return Err("broker port unavailable".into());
        }
        Ok(listener)
    }
}
fn serve(
    mut client: TcpStream,
    peer_port: u16,
    port: u16,
    package: &VerifiedPackage,
    job: &Job,
    healthy: &AtomicBool,
    connected: &AtomicBool,
) -> Result<(), String> {
    let pid = telemetry::broker_peer(peer_port, port)?;
    if !job.contains(pid) {
        return Err("broker client is not supervised".into());
    }
    let image = crate::platform::image(pid)?;
    if !package.manifest().network.programs.iter().any(|p| {
        p.role == ProgramRole::SteamBroker
            && platform::same_image(&package.root().join(&p.path), &image)
    }) {
        return Err("broker client role rejected".into());
    }
    configure_client(&client)?;
    let mut header = vec![];
    let mut byte = [0];
    let deadline = Instant::now() + Duration::from_secs(5);
    while !header.ends_with(b"\r\n\r\n") && header.len() < 8192 {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("broker request deadline exceeded")?;
        client
            .set_read_timeout(Some(remaining))
            .map_err(|_| "broker timeout unavailable")?;
        if client
            .read(&mut byte)
            .map_err(|_| "broker request unavailable")?
            != 1
        {
            return Err("broker request interrupted".into());
        }
        header.push(byte[0]);
    }
    let host = match connect_host(&header) {
        Ok(host) => host,
        Err(error) => {
            let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
            return Err(error);
        }
    };
    if !healthy.load(Ordering::Acquire) {
        return Err("supervision unavailable".into());
    }
    // Resolve once, validate all answers, and connect to the checked address itself (no second DNS lookup).
    let addresses: Vec<_> = (host.as_str(), 443)
        .to_socket_addrs()
        .map_err(|_| "approved destination DNS failed")?
        .take(32)
        .collect();
    if addresses.is_empty() || addresses.iter().any(|address| !public_ip(address.ip())) {
        return Err("nonpublic destination rejected".into());
    }
    let mut upstream = None;
    for address in addresses {
        if let Ok(stream) = TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
            upstream = Some(stream);
            break;
        }
    }
    let mut upstream = upstream.ok_or("approved destination unavailable")?;
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .map_err(|_| "broker client disconnected")?;
    if package.manifest().mode == "development" && !connected.swap(true, Ordering::Relaxed) {
        eprintln!("Steam broker established an approved tunnel");
    }
    client
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|_| "broker timeout unavailable")?;
    upstream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|_| "broker timeout unavailable")?;
    upstream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|_| "broker timeout unavailable")?;
    let mut client_read = client
        .try_clone()
        .map_err(|_| "broker stream unavailable")?;
    let mut upstream_write = upstream
        .try_clone()
        .map_err(|_| "broker stream unavailable")?;
    let copy = std::thread::spawn(move || {
        let result = std::io::copy(&mut client_read, &mut upstream_write);
        let _ = upstream_write.shutdown(Shutdown::Write);
        result
    });
    let result = std::io::copy(&mut upstream, &mut client);
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.shutdown(Shutdown::Both);
    let outbound = copy.join();
    result.map_err(|_| "upstream tunnel transfer failed")?;
    outbound
        .map_err(|_| "client tunnel transfer failed")?
        .map_err(|_| "client tunnel transfer failed")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_tunnel_waits_for_delayed_connect_and_payload() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut client, _) = listener.accept().unwrap();
        client.set_nonblocking(true).unwrap();
        assert_eq!(
            client.read(&mut [0]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        configure_client(&client).unwrap();
        let payload = b"CONNECT partner.steamgames.com:443 HTTP/1.1\r\nHost: partner.steamgames.com:443\r\n\r\n";
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            peer.write_all(payload).unwrap();
            let mut reply = [0; 4];
            peer.read_exact(&mut reply).unwrap();
            assert_eq!(&reply, b"TEST");
            std::thread::sleep(Duration::from_millis(60));
            peer.write_all(b"DATA").unwrap();
        });
        let mut frame = vec![0; payload.len()];
        client.read_exact(&mut frame).unwrap();
        assert_eq!(connect_host(&frame).unwrap(), "partner.steamgames.com");
        client.write_all(b"TEST").unwrap();
        let mut data = [0; 4];
        client.read_exact(&mut data).unwrap();
        assert_eq!(&data, b"DATA");
        writer.join().unwrap();
    }
}
