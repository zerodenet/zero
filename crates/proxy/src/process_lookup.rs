//! Resolve local process identity from a source socket address.
//!
//! On Linux this reads `/proc/net/tcp` to map (local_ip, local_port) → inode,
//! then scans `/proc/<pid>/fd` to find the owning process.
//! Other platforms return `None` (process_id and process_name stay empty).

use std::io;
use std::net::SocketAddr;

/// Process identity resolved from a source address.
pub(crate) struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

/// Attempt to identify the local process that owns `source_addr`.
///
/// Returns `None` on non-Linux platforms or if the lookup fails
/// (process already exited, /proc not mounted, etc.).
pub(crate) fn lookup_process(source_addr: SocketAddr) -> Option<ProcessInfo> {
    #[cfg(target_os = "linux")]
    return lookup_process_linux(source_addr);

    #[cfg(not(target_os = "linux"))]
    {
        let _ = source_addr;
        None
    }
}

#[cfg(target_os = "linux")]
fn lookup_process_linux(source_addr: SocketAddr) -> Option<ProcessInfo> {
    let inode = find_socket_inode(source_addr)?;
    find_process_by_inode(inode)
}

/// Parse `/proc/net/tcp` to find the inode for a given (ip, port).
#[cfg(target_os = "linux")]
fn find_socket_inode(addr: SocketAddr) -> Option<u64> {
    let tcp = std::fs::read_to_string("/proc/net/tcp").ok()?;
    let ip = match addr.ip() {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            // /proc/net/tcp uses little-endian hex for IP
            format!("{:02X}{:02X}{:02X}{:02X}", octets[3], octets[2], octets[1], octets[0])
        }
        std::net::IpAddr::V6(_) => return None, // v6 is in /proc/net/tcp6, skip for now
    };
    let port = format!("{:04X}", addr.port());

    for line in tcp.lines().skip(1) {
        // Columns: sl local_address rem_address st tx_queue rx_queue tr tm->when retrnsmt uid timeout inode
        let mut fields = line.split_whitespace();
        fields.next()?; // sl
        let local = fields.next()?; // local_address (IP:PORT hex)
        let mut local_parts = local.split(':');
        let local_ip = local_parts.next()?;
        let local_port = local_parts.next()?;

        if local_ip == ip && local_port == port {
            // Skip to inode (10th field, index 9)
            for _ in 0..7 {
                fields.next()?;
            }
            let inode: u64 = fields.next()?.parse().ok()?;
            return Some(inode);
        }
    }
    None
}

/// Scan `/proc/<pid>/fd/` to find the process that owns a socket inode.
#[cfg(target_os = "linux")]
fn find_process_by_inode(target_inode: u64) -> Option<ProcessInfo> {
    // Scan /proc for processes
    let proc_dir = std::fs::read_dir("/proc").ok()?;

    for entry in proc_dir {
        let entry = entry.ok()?;
        let pid_str = entry.file_name();
        let pid: u32 = pid_str.to_str()?.parse().ok()?;
        if pid == 0 { continue; }

        // Check /proc/<pid>/fd/ for socket links
        let fd_dir = match std::fs::read_dir(format!("/proc/{pid}/fd")) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for fd_entry in fd_dir {
            let fd_entry = match fd_entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let link = match std::fs::read_link(fd_entry.path()) {
                Ok(l) => l,
                Err(_) => continue,
            };
            let link_str = link.to_string_lossy();
            if link_str.starts_with("socket:[") && link_str.contains(&format!("{}", target_inode)) {
                // Found the process — read its name from /proc/<pid>/comm
                let name = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_else(|_| "unknown".to_owned());
                return Some(ProcessInfo { pid, name });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn parse_tcp_line() {
        // Verify we can parse a real /proc/net/tcp file
        let tcp = std::fs::read_to_string("/proc/net/tcp").unwrap();
        assert!(!tcp.is_empty(), "/proc/net/tcp should be readable");
        // Should have header + at least one entry
        assert!(tcp.lines().count() > 1, "should have entries");
    }
}
