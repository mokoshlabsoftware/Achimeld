use serde::Serialize;
use std::{collections::HashSet, ffi::c_void, ptr};
use windows_sys::Win32::{NetworkManagement::IpHelper::*, Networking::WinSock::*};

pub const COVERAGE: &str = "Windows TCP/UDP process tables sampled every second, IPv4 and IPv6. TCP state and destinations; UDP local bindings only. Short connections, denied attempts, payloads and delegated traffic are not observed. Zero observations do not prove zero disclosure.";
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub process_id: u32,
    pub protocol: &'static str,
    pub destination: Option<String>,
    pub port: Option<u16>,
    pub state: Option<u32>,
    pub observed_unix: u64,
}
fn table(tcp: bool, ipv6: bool) -> Result<Vec<u8>, String> {
    let family = if ipv6 { AF_INET6 } else { AF_INET } as u32;
    let mut size = 0;
    // SAFETY: Callers provide either a null size-query pointer or aligned, initialized storage of the input size; size output is live.
    let call = |buffer: *mut c_void, size: &mut u32| unsafe {
        if tcp {
            GetExtendedTcpTable(buffer, size, 0, family, TCP_TABLE_OWNER_PID_ALL, 0)
        } else {
            GetExtendedUdpTable(buffer, size, 0, family, UDP_TABLE_OWNER_PID, 0)
        }
    };
    let code = call(ptr::null_mut(), &mut size);
    if code != 122 && code != 0 {
        return Err("Windows connection collector unavailable".into());
    }
    for _ in 0..3 {
        if !(4..=16 * 1024 * 1024).contains(&size) {
            return Err("Windows connection table limit exceeded".into());
        }
        // Windows table APIs write aligned native structs, not arbitrary byte storage.
        let mut storage = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        let code = call(storage.as_mut_ptr() as *mut c_void, &mut size);
        if code == 0 {
            if size < 4 || size as usize > storage.len() * size_of::<usize>() {
                return Err("Windows connection table malformed".into());
            }
            // SAFETY: Initialized storage is live and the successful returned size is checked within its allocated byte capacity.
            return Ok(unsafe {
                std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), size as usize)
            }
            .to_vec());
        }
        if code != 122 {
            return Err("Windows connection collection failed".into());
        }
    }
    Err("Windows connection table changed too quickly".into())
}
// These four private row types contain only integer/byte fields, permitting all bit patterns.
// Do not generalize this bound to arbitrary Copy types (e.g. bool, references, enums).
trait TableRow: Copy {}
impl TableRow for MIB_TCPROW_OWNER_PID {}
impl TableRow for MIB_TCP6ROW_OWNER_PID {}
impl TableRow for MIB_UDPROW_OWNER_PID {}
impl TableRow for MIB_UDP6ROW_OWNER_PID {}
fn rows<T: TableRow>(buffer: &[u8]) -> Result<Vec<T>, String> {
    let header = buffer
        .get(..4)
        .ok_or("Windows connection table malformed")?;
    let count = u32::from_ne_bytes(
        header
            .try_into()
            .map_err(|_| "Windows connection table malformed")?,
    ) as usize;
    if count > (buffer.len() - 4) / size_of::<T>() {
        return Err("Windows connection table malformed".into());
    }
    Ok((0..count)
        // SAFETY: Private TableRow types permit all bit patterns; validated count fits the buffer and read_unaligned removes alignment requirements.
        .map(|index| unsafe {
            ptr::read_unaligned(buffer.as_ptr().add(4 + index * size_of::<T>()) as *const T)
        })
        .collect())
}
fn port(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}
pub fn collect(pids: &HashSet<u32>) -> Result<Vec<Observation>, String> {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock unavailable")?
        .as_secs();
    let mut result = vec![];
    for row in rows::<MIB_TCPROW_OWNER_PID>(&table(true, false)?)? {
        if pids.contains(&row.dwOwningPid) {
            result.push(Observation {
                process_id: row.dwOwningPid,
                protocol: "tcp4",
                destination: Some(
                    std::net::Ipv4Addr::from(row.dwRemoteAddr.to_ne_bytes()).to_string(),
                ),
                port: Some(port(row.dwRemotePort)),
                state: Some(row.dwState),
                observed_unix: time,
            });
        }
    }
    for row in rows::<MIB_TCP6ROW_OWNER_PID>(&table(true, true)?)? {
        if pids.contains(&row.dwOwningPid) {
            result.push(Observation {
                process_id: row.dwOwningPid,
                protocol: "tcp6",
                destination: Some(std::net::Ipv6Addr::from(row.ucRemoteAddr).to_string()),
                port: Some(port(row.dwRemotePort)),
                state: Some(row.dwState),
                observed_unix: time,
            });
        }
    }
    for row in rows::<MIB_UDPROW_OWNER_PID>(&table(false, false)?)? {
        if pids.contains(&row.dwOwningPid) {
            result.push(Observation {
                process_id: row.dwOwningPid,
                protocol: "udp4",
                destination: None,
                port: None,
                state: None,
                observed_unix: time,
            });
        }
    }
    for row in rows::<MIB_UDP6ROW_OWNER_PID>(&table(false, true)?)? {
        if pids.contains(&row.dwOwningPid) {
            result.push(Observation {
                process_id: row.dwOwningPid,
                protocol: "udp6",
                destination: None,
                port: None,
                state: None,
                observed_unix: time,
            });
        }
    }
    Ok(result)
}
pub fn broker_peer(peer_port: u16, broker_port: u16) -> Result<u32, String> {
    let candidates: Vec<_> = rows::<MIB_TCPROW_OWNER_PID>(&table(true, false)?)?
        .into_iter()
        .filter(|row| {
            port(row.dwLocalPort) == peer_port
                && port(row.dwRemotePort) == broker_port
                && row.dwLocalAddr.to_ne_bytes() == [127, 0, 0, 1]
                && row.dwRemoteAddr.to_ne_bytes() == [127, 0, 0, 1]
        })
        .collect();
    if candidates.len() != 1 {
        return Err("broker client attribution unavailable".into());
    }
    Ok(candidates[0].dwOwningPid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_connection_tables_are_rejected_without_reading_out_of_bounds() {
        for size in 0..4 {
            assert!(rows::<MIB_TCPROW_OWNER_PID>(&vec![0; size]).is_err());
        }
        assert!(rows::<MIB_TCPROW_OWNER_PID>(&1u32.to_ne_bytes()).is_err());
        assert!(rows::<MIB_TCPROW_OWNER_PID>(&u32::MAX.to_ne_bytes()).is_err());
        assert!(
            rows::<MIB_TCPROW_OWNER_PID>(&0u32.to_ne_bytes())
                .unwrap()
                .is_empty()
        );
    }
}
