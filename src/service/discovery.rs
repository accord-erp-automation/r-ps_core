use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::Duration;

use super::config::DEFAULT_DISCOVERY_PORT;
use super::mobile_contract::{DiscoveryAnnouncement, ServiceIdentity};

pub const DISCOVERY_PROBE_V1: &str = "GSCALE_DISCOVER_V1";
pub const DISCOVERY_ANNOUNCE_INTERVAL_MS: u64 = 250;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoverySocketConfig {
    pub bind_addr: SocketAddrV4,
    pub announce_targets: Vec<SocketAddrV4>,
    pub read_timeout: Duration,
}

impl DiscoverySocketConfig {
    pub fn new(bind_ip: Ipv4Addr, discovery_port: u16, announce_targets: Vec<Ipv4Addr>) -> Self {
        let port = if discovery_port == 0 {
            DEFAULT_DISCOVERY_PORT
        } else {
            discovery_port
        };
        let announce_targets = normalize_announce_targets(announce_targets, port);

        Self {
            bind_addr: SocketAddrV4::new(bind_ip, port),
            announce_targets,
            read_timeout: Duration::from_millis(250),
        }
    }

    pub fn with_socket_targets(
        bind_ip: Ipv4Addr,
        discovery_port: u16,
        announce_targets: Vec<SocketAddrV4>,
    ) -> Self {
        let port = if discovery_port == 0 {
            DEFAULT_DISCOVERY_PORT
        } else {
            discovery_port
        };
        let announce_targets = if announce_targets.is_empty() {
            normalize_announce_targets(Vec::new(), port)
        } else {
            dedupe_socket_targets(announce_targets)
        };

        Self {
            bind_addr: SocketAddrV4::new(bind_ip, port),
            announce_targets,
            read_timeout: Duration::from_millis(250),
        }
    }
}

pub fn discovery_response_for_packet(
    packet: &[u8],
    identity: &ServiceIdentity,
    http_port: u16,
    candidate_ports: Vec<u16>,
) -> Option<Vec<u8>> {
    if !is_discovery_probe(packet) {
        return None;
    }
    DiscoveryAnnouncement::new(identity, http_port, candidate_ports)
        .to_json_bytes()
        .ok()
}

pub fn is_discovery_probe(packet: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(packet) else {
        return false;
    };
    text.trim() == DISCOVERY_PROBE_V1
}

pub fn bind_discovery_socket(config: &DiscoverySocketConfig) -> io::Result<UdpSocket> {
    let socket = bind_reusable_udp_socket(config.bind_addr)?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(config.read_timeout))?;
    Ok(socket)
}

pub fn bind_announcement_socket() -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    Ok(socket)
}

pub fn send_discovery_announcement(
    socket: &UdpSocket,
    config: &DiscoverySocketConfig,
    identity: &ServiceIdentity,
    http_port: u16,
    candidate_ports: Vec<u16>,
) -> io::Result<usize> {
    let payload = DiscoveryAnnouncement::new(identity, http_port, candidate_ports)
        .to_json_bytes()
        .map_err(io::Error::other)?;
    let mut sent = 0;
    for target in &config.announce_targets {
        socket.send_to(&payload, target)?;
        sent += 1;
    }
    Ok(sent)
}

pub fn broadcast_targets_from_ipv4_networks(
    networks: &[(Ipv4Addr, Ipv4Addr)],
    discovery_port: u16,
) -> Vec<SocketAddrV4> {
    let port = if discovery_port == 0 {
        DEFAULT_DISCOVERY_PORT
    } else {
        discovery_port
    };
    let mut targets = vec![SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), port)];

    for (ip, mask) in networks {
        if !is_private_ipv4(*ip) {
            continue;
        }
        let broadcast = ipv4_broadcast(*ip, *mask);
        let target = SocketAddrV4::new(broadcast, port);
        if !targets.contains(&target) {
            targets.push(target);
        }
    }

    targets
}

pub fn collect_discovery_broadcast_targets(discovery_port: u16) -> Vec<SocketAddrV4> {
    collect_interface_ipv4_networks()
        .map(|networks| broadcast_targets_from_ipv4_networks(&networks, discovery_port))
        .unwrap_or_else(|| {
            let port = if discovery_port == 0 {
                DEFAULT_DISCOVERY_PORT
            } else {
                discovery_port
            };
            vec![SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), port)]
        })
}

fn normalize_announce_targets(
    announce_targets: Vec<Ipv4Addr>,
    discovery_port: u16,
) -> Vec<SocketAddrV4> {
    if announce_targets.is_empty() {
        return vec![SocketAddrV4::new(
            Ipv4Addr::new(255, 255, 255, 255),
            discovery_port,
        )];
    }

    let mut out = Vec::new();
    for target in announce_targets {
        let socket = SocketAddrV4::new(target, discovery_port);
        if !out.contains(&socket) {
            out.push(socket);
        }
    }
    out
}

fn dedupe_socket_targets(announce_targets: Vec<SocketAddrV4>) -> Vec<SocketAddrV4> {
    let mut out = Vec::new();
    for target in announce_targets {
        if !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

#[cfg(unix)]
fn bind_reusable_udp_socket(addr: SocketAddrV4) -> io::Result<UdpSocket> {
    use std::mem;
    use std::os::fd::FromRawFd;

    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    if let Err(err) = set_socket_reuse(fd) {
        close_fd(fd);
        return Err(err);
    }

    let raw_addr = socket_addr_v4_to_raw(addr);
    let bind_result = unsafe {
        libc::bind(
            fd,
            (&raw_addr as *const libc::sockaddr_in).cast::<libc::sockaddr>(),
            mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };
    if bind_result != 0 {
        let err = io::Error::last_os_error();
        close_fd(fd);
        return Err(err);
    }

    Ok(unsafe { UdpSocket::from_raw_fd(fd) })
}

#[cfg(not(unix))]
fn bind_reusable_udp_socket(addr: SocketAddrV4) -> io::Result<UdpSocket> {
    UdpSocket::bind(addr)
}

#[cfg(unix)]
fn set_socket_reuse(fd: libc::c_int) -> io::Result<()> {
    set_socket_flag(fd, libc::SO_REUSEADDR)?;
    set_reuse_port_if_supported(fd)
}

#[cfg(all(unix, any(target_os = "macos", target_os = "ios", target_os = "linux")))]
fn set_reuse_port_if_supported(fd: libc::c_int) -> io::Result<()> {
    set_socket_flag(fd, libc::SO_REUSEPORT)
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "linux"))
))]
fn set_reuse_port_if_supported(_fd: libc::c_int) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_socket_flag(fd: libc::c_int, option: libc::c_int) -> io::Result<()> {
    let enabled: libc::c_int = 1;
    let result = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            option,
            (&enabled as *const libc::c_int).cast::<libc::c_void>(),
            std::mem::size_of_val(&enabled) as libc::socklen_t,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn close_fd(fd: libc::c_int) {
    let _ = unsafe { libc::close(fd) };
}

#[cfg(unix)]
fn socket_addr_v4_to_raw(addr: SocketAddrV4) -> libc::sockaddr_in {
    libc::sockaddr_in {
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: addr.port().to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(addr.ip().octets()),
        },
        sin_zero: [0; 8],
    }
}

#[cfg(unix)]
fn collect_interface_ipv4_networks() -> Option<Vec<(Ipv4Addr, Ipv4Addr)>> {
    use std::ptr;

    let mut addrs: *mut libc::ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut addrs) } != 0 {
        return None;
    }

    let mut out = Vec::new();
    let mut cursor = addrs;
    while !cursor.is_null() {
        let iface = unsafe { &*cursor };
        let flags = iface.ifa_flags as i32;
        let is_up = flags & libc::IFF_UP != 0;
        let is_loopback = flags & libc::IFF_LOOPBACK != 0;
        if is_up
            && !is_loopback
            && let Some(ip) = sockaddr_ipv4(iface.ifa_addr)
            && let Some(mask) = sockaddr_ipv4(iface.ifa_netmask)
        {
            out.push((ip, mask));
        }
        cursor = iface.ifa_next;
    }

    unsafe { libc::freeifaddrs(addrs) };
    Some(out)
}

#[cfg(not(unix))]
fn collect_interface_ipv4_networks() -> Option<Vec<(Ipv4Addr, Ipv4Addr)>> {
    None
}

#[cfg(unix)]
fn sockaddr_ipv4(addr: *const libc::sockaddr) -> Option<Ipv4Addr> {
    if addr.is_null() {
        return None;
    }
    let sockaddr = unsafe { &*addr };
    if sockaddr.sa_family as i32 != libc::AF_INET {
        return None;
    }
    let addr_in = unsafe { &*(addr as *const libc::sockaddr_in) };
    Some(Ipv4Addr::from(addr_in.sin_addr.s_addr.to_ne_bytes()))
}

fn ipv4_broadcast(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    let ip = u32::from(ip);
    let mask = u32::from(mask);
    Ipv4Addr::from(ip | !mask)
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 10
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn identity() -> ServiceIdentity {
        ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin")
    }

    #[test]
    fn recognizes_exact_gscale_probe_with_whitespace() {
        assert!(is_discovery_probe(b"GSCALE_DISCOVER_V1"));
        assert!(is_discovery_probe(b" GSCALE_DISCOVER_V1\n"));
        assert!(!is_discovery_probe(b"GSCALE_DISCOVER_V2"));
        assert!(!is_discovery_probe(&[0xff, 0xfe]));
    }

    #[test]
    fn returns_announcement_only_for_probe_packet() {
        let response =
            discovery_response_for_packet(b"GSCALE_DISCOVER_V1", &identity(), 39117, vec![39117])
                .unwrap();
        let decoded: Value = serde_json::from_slice(&response).unwrap();

        assert_eq!(decoded["type"], "gscale_announce_v1");
        assert_eq!(decoded["service"], "mobileapi");
        assert_eq!(decoded["server_name"], "rp-scale");
        assert_eq!(decoded["http_port"], 39117);
        assert!(discovery_response_for_packet(b"unknown", &identity(), 39117, vec![]).is_none());
    }

    #[test]
    fn computes_broadcast_targets_like_gscale_mobileapi() {
        let targets = broadcast_targets_from_ipv4_networks(
            &[
                (
                    Ipv4Addr::new(192, 168, 1, 10),
                    Ipv4Addr::new(255, 255, 255, 0),
                ),
                (Ipv4Addr::new(10, 42, 0, 80), Ipv4Addr::new(255, 255, 0, 0)),
                (Ipv4Addr::new(8, 8, 8, 8), Ipv4Addr::new(255, 255, 255, 0)),
            ],
            18081,
        );

        assert!(targets.contains(&SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 18081)));
        assert!(targets.contains(&SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 255), 18081)));
        assert!(targets.contains(&SocketAddrV4::new(Ipv4Addr::new(10, 42, 255, 255), 18081)));
        assert_eq!(targets.len(), 3);
    }

    #[test]
    fn socket_config_defaults_to_gscale_discovery_port_and_broadcast() {
        let config = DiscoverySocketConfig::new(Ipv4Addr::UNSPECIFIED, 0, vec![]);

        assert_eq!(
            config.bind_addr,
            SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 18081)
        );
        assert_eq!(
            config.announce_targets,
            vec![SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 18081)]
        );
    }

    #[test]
    fn socket_config_accepts_subnet_broadcast_targets() {
        let config = DiscoverySocketConfig::with_socket_targets(
            Ipv4Addr::UNSPECIFIED,
            18081,
            vec![
                SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 18081),
                SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 255), 18081),
                SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 255), 18081),
            ],
        );

        assert_eq!(config.announce_targets.len(), 2);
        assert!(
            config
                .announce_targets
                .contains(&SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 255), 18081))
        );
    }

    #[test]
    fn announcement_socket_uses_ephemeral_port_like_gscale() {
        let socket = bind_announcement_socket().unwrap();
        let local = socket.local_addr().unwrap();

        assert_ne!(local.port(), DEFAULT_DISCOVERY_PORT);
    }

    #[test]
    fn discovery_socket_allows_multiple_binds_on_same_port() {
        let first_config = DiscoverySocketConfig::new(Ipv4Addr::LOCALHOST, 0, vec![]);
        let first = bind_discovery_socket(&first_config).unwrap();
        let port = first.local_addr().unwrap().port();
        let second_config = DiscoverySocketConfig::new(Ipv4Addr::LOCALHOST, port, vec![]);
        let second = bind_discovery_socket(&second_config).unwrap();

        assert_eq!(second.local_addr().unwrap().port(), port);
    }
}
