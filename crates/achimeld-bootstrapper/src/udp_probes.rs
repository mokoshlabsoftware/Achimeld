//! Independent, exclusive local receivers distinguish UDP delivery from a send
//! merely being accepted into the operating system's asynchronous queue.
use std::{
    collections::VecDeque,
    net::{SocketAddr, UdpSocket},
    os::windows::io::FromRawSocket,
    time::{Duration, Instant},
};
use windows_sys::Win32::Networking::WinSock::*;

type Pending = VecDeque<(Instant, [u8; 32])>;
pub struct Probes {
    ipv4: UdpSocket,
    ipv6: UdpSocket,
    pending4: Pending,
    pending6: Pending,
    pub port: u16,
}
impl Probes {
    pub fn create() -> Result<Self, String> {
        let ipv4 = bind(false, 0)?;
        let port = ipv4
            .local_addr()
            .map_err(|_| "UDP probe identity unavailable")?
            .port();
        let ipv6 = bind(true, port)?;
        let probes = Self {
            ipv4,
            ipv6,
            pending4: Pending::new(),
            pending6: Pending::new(),
            port,
        };
        for socket in [&probes.ipv4, &probes.ipv6] {
            let token = rand::random::<[u8; 32]>();
            socket
                .send_to(
                    &token,
                    socket
                        .local_addr()
                        .map_err(|_| "UDP probe identity unavailable")?,
                )
                .map_err(|_| "UDP probe positive control unavailable")?;
            if !received(socket, &mut Pending::new(), &token)? {
                return Err("UDP probe positive control did not arrive".into());
            }
        }
        Ok(probes)
    }
    pub fn denied(&mut self, ipv6: bool, token: &str) -> Result<bool, String> {
        let token = hex::decode(token).map_err(|_| "invalid UDP probe token")?;
        if token.len() != 32 {
            return Err("invalid UDP probe token".into());
        }
        let (socket, pending) = if ipv6 {
            (&self.ipv6, &mut self.pending6)
        } else {
            (&self.ipv4, &mut self.pending4)
        };
        if received(socket, pending, &token)? {
            return Ok(false);
        }
        let control = rand::random::<[u8; 32]>();
        socket
            .send_to(
                &control,
                socket
                    .local_addr()
                    .map_err(|_| "UDP probe identity unavailable")?,
            )
            .map_err(|_| "UDP probe positive control unavailable")?;
        if !received(socket, pending, &control)? {
            return Err("UDP probe positive control did not arrive".into());
        }
        // Preserve other clients' tokens, including a late delivery, rather than
        // incorrectly reporting denial after consuming somebody else's datagram.
        Ok(!pending
            .iter()
            .any(|(_, value)| value.as_slice() == token.as_slice()))
    }
}
fn bind(ipv6: bool, port: u16) -> Result<UdpSocket, String> {
    // SAFETY: Winsock structs and option buffers are live; successful socket transfers once to UdpSocket before any fallible bind/configuration.
    unsafe {
        let mut data = WSADATA::default();
        if WSAStartup(0x0202, &mut data) != 0 {
            return Err("UDP probe sockets unavailable".into());
        }
        let raw = socket(
            if ipv6 { AF_INET6 } else { AF_INET } as i32,
            SOCK_DGRAM,
            IPPROTO_UDP,
        );
        if raw == INVALID_SOCKET {
            return Err("UDP probe socket unavailable".into());
        }
        let socket = UdpSocket::from_raw_socket(raw as u64);
        let exclusive = 1_i32;
        if setsockopt(
            raw,
            SOL_SOCKET,
            SO_EXCLUSIVEADDRUSE,
            &exclusive as *const _ as *const u8,
            size_of::<i32>() as i32,
        ) != 0
        {
            return Err("exclusive UDP probe unavailable".into());
        }
        let address: SocketAddr = if ipv6 {
            ([0, 0, 0, 0, 0, 0, 0, 1], port).into()
        } else {
            ([127, 0, 0, 1], port).into()
        };
        let result = match address {
            SocketAddr::V4(address) => {
                let value = SOCKADDR_IN {
                    sin_family: AF_INET,
                    sin_port: port.to_be(),
                    sin_addr: IN_ADDR {
                        S_un: IN_ADDR_0 {
                            S_addr: u32::from_ne_bytes(address.ip().octets()),
                        },
                    },
                    ..Default::default()
                };
                windows_sys::Win32::Networking::WinSock::bind(
                    raw,
                    &value as *const _ as *const SOCKADDR,
                    size_of::<SOCKADDR_IN>() as i32,
                )
            }
            SocketAddr::V6(address) => {
                let value = SOCKADDR_IN6 {
                    sin6_family: AF_INET6,
                    sin6_port: port.to_be(),
                    sin6_addr: IN6_ADDR {
                        u: IN6_ADDR_0 {
                            Byte: address.ip().octets(),
                        },
                    },
                    ..Default::default()
                };
                windows_sys::Win32::Networking::WinSock::bind(
                    raw,
                    &value as *const _ as *const SOCKADDR,
                    size_of::<SOCKADDR_IN6>() as i32,
                )
            }
        };
        if result != 0 {
            return Err("UDP probe endpoint unavailable".into());
        }
        socket
            .set_nonblocking(true)
            .map_err(|_| "UDP probe timeout unavailable")?;
        Ok(socket)
    }
}
fn received(socket: &UdpSocket, pending: &mut Pending, token: &[u8]) -> Result<bool, String> {
    pending.retain(|(time, _)| time.elapsed() < Duration::from_secs(5));
    if let Some(index) = pending
        .iter()
        .position(|(_, value)| value.as_slice() == token)
    {
        pending.remove(index);
        return Ok(true);
    }
    let deadline = Instant::now() + Duration::from_millis(200);
    let mut buffer = [0; 64];
    let mut count = 0;
    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((length, peer)) => {
                if peer.ip().is_loopback() && length == 32 {
                    if &buffer[..length] == token {
                        return Ok(true);
                    }
                    if pending.len() >= 128 {
                        return Err("UDP probe input limit exceeded".into());
                    }
                    pending.push_back((Instant::now(), buffer[..32].try_into().unwrap()));
                }
                count += 1;
                if count > 128 {
                    return Err("UDP probe input limit exceeded".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(_) => return Err("UDP probe receiver failed".into()),
        }
    }
    Ok(false)
}
#[cfg(test)]
mod tests {
    #[test]
    fn independent_receivers_detect_delivery_and_missing_tokens() {
        let mut probes = super::Probes::create().unwrap();
        for ipv6 in [false, true] {
            let socket = if ipv6 { &probes.ipv6 } else { &probes.ipv4 };
            let token = rand::random::<[u8; 32]>();
            socket
                .send_to(&token, socket.local_addr().unwrap())
                .unwrap();
            assert!(!probes.denied(ipv6, &hex::encode(token)).unwrap());
            assert!(
                probes
                    .denied(ipv6, &hex::encode(rand::random::<[u8; 32]>()))
                    .unwrap()
            );
        }
    }
    #[test]
    fn interleaved_client_tokens_are_not_discarded() {
        let mut probes = super::Probes::create().unwrap();
        let first = rand::random::<[u8; 32]>();
        let second = rand::random::<[u8; 32]>();
        probes
            .ipv4
            .send_to(&first, probes.ipv4.local_addr().unwrap())
            .unwrap();
        probes
            .ipv4
            .send_to(&second, probes.ipv4.local_addr().unwrap())
            .unwrap();
        assert!(!probes.denied(false, &hex::encode(second)).unwrap());
        assert!(!probes.denied(false, &hex::encode(first)).unwrap());
    }
}
