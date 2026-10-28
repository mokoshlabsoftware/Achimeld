//! Pure broker request and destination policy.
use std::net::IpAddr;

pub fn allowed_host(host: &str) -> bool {
    if host.len() > 253
        || host.is_empty()
        || !host.is_ascii()
        || host.starts_with('.')
        || host.ends_with('.')
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return false;
    }
    let host = host.to_ascii_lowercase();
    matches!(
        host.as_str(),
        "partner.steamgames.com"
            | "store.steampowered.com"
            | "login.steampowered.com"
            | "api.steampowered.com"
            | "steamcommunity.com"
    ) || ["steamstatic.com", "steamcontent.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let a = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || a[0] == 0
                || a[0] >= 240
                || (a[0] == 100 && (64..=127).contains(&a[1]))
                || (a[0] == 192 && a[1] == 0)
                || (a[0] == 198 && (a[1] == 18 || a[1] == 19 || (a[1] == 51 && a[2] == 100)))
                || (a[0] == 203 && a[1] == 0 && a[2] == 113))
        }
        IpAddr::V6(ip) => {
            let a = ip.segments();
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            // Only globally routed unicast; exclude documentation, transition and local ranges.
            a[0] & 0xe000 == 0x2000
                && a[0] != 0x2002
                && !(a[0] == 0x2001 && (a[1] < 0x0200 || a[1] == 0x0db8))
                // RFC 9637 / IANA: 3fff::/20 is documentation, not public routing.
                && !(a[0] == 0x3fff && a[1] & 0xf000 == 0)
        }
    }
}
pub fn connect_host(header: &[u8]) -> Result<String, String> {
    if header.len() > 8192 || !header.ends_with(b"\r\n\r\n") {
        return Err("invalid broker request framing".into());
    }
    let text = std::str::from_utf8(header).map_err(|_| "invalid broker request encoding")?;
    if text.bytes().any(|b| b < 32 && !matches!(b, b'\r' | b'\n')) {
        return Err("invalid broker request characters".into());
    }
    let mut lines = text[..text.len() - 4].split("\r\n");
    let first = lines.next().ok_or("empty broker request")?;
    let parts: Vec<_> = first.split(' ').collect();
    if parts.len() != 3 || parts[0] != "CONNECT" || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return Err("only HTTPS tunnels are permitted".into());
    }
    let host = parts[1]
        .strip_suffix(":443")
        .ok_or("only HTTPS port 443 is permitted")?
        .to_ascii_lowercase();
    if !allowed_host(&host) {
        return Err("destination is not approved".into());
    }
    let mut seen = std::collections::HashSet::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or("invalid broker request header")?;
        let name = name.to_ascii_lowercase();
        if !seen.insert(name.clone())
            || !matches!(
                name.as_str(),
                "host" | "proxy-connection" | "user-agent" | "connection"
            )
        {
            return Err("unapproved broker request header".into());
        }
        if name == "host" && !value.trim().eq_ignore_ascii_case(parts[1]) {
            return Err("broker request host mismatch".into());
        }
    }
    Ok(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_mutations_cannot_bypass_connect_destination_policy() {
        let seed = b"CONNECT partner.steamgames.com:443 HTTP/1.1\r\nHost: partner.steamgames.com:443\r\n\r\n";
        for position in 0..seed.len() {
            for replacement in [0, b' ', b'/', b':', b'\r', b'\n', b'.', 0xff] {
                let mut input = seed.to_vec();
                input[position] = replacement;
                if let Ok(host) = connect_host(&input) {
                    assert!(allowed_host(&host));
                    assert_eq!(host, "partner.steamgames.com");
                }
            }
        }
        for host in [
            "steamstatic.com.evil",
            "evilsteamstatic.com",
            "steamstatic.com.",
            "127.0.0.1",
            "[::1]",
        ] {
            assert!(
                connect_host(format!("CONNECT {host}:443 HTTP/1.1\r\n\r\n").as_bytes()).is_err()
            );
        }
    }
    #[test]
    fn bounded_connect_only() {
        assert_eq!(
            connect_host(b"CONNECT api.steampowered.com:443 HTTP/1.1\r\n\r\n").unwrap(),
            "api.steampowered.com"
        );
        assert!(!allowed_host("sub.api.steampowered.com"));
        assert!(!allowed_host("api.steampowered.com.evil.test"));
        assert_eq!(connect_host(b"CONNECT partner.steamgames.com:443 HTTP/1.1\r\nHost: partner.steamgames.com:443\r\n\r\n").unwrap(),"partner.steamgames.com");
        for bad in [
            "GET https://partner.steamgames.com/ HTTP/1.1\r\n\r\n",
            "CONNECT partner.steamgames.com:80 HTTP/1.1\r\n\r\n",
            "CONNECT partner.steamgames.com.evil.test:443 HTTP/1.1\r\n\r\n",
            "CONNECT partner.steamgames.com:443 HTTP/1.1\r\nContent-Length: 10\r\n\r\n",
            "CONNECT partner.steamgames.com:443 HTTP/1.1\r\nHost: evil.test:443\r\n\r\n",
        ] {
            assert!(connect_host(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn private_destinations_rejected() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "192.168.1.1",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "3fff::1",
            "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
        ] {
            assert!(!public_ip(address.parse().unwrap()), "{address}");
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
