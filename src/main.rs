use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> std::io::Result<()> {
    let filepath = Path::new("/etc/hosts");
    let file = File::open(filepath)?;
    let reader = io::BufReader::new(file);

    let tmp_dir = std::env::temp_dir();
    let tmp_hosts = if let Some(file_name) = filepath.file_name() {
        tmp_dir.join(file_name)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No file name in path",
        ));
    };

    let mut ip_host_map: HashMap<String, Vec<String>> = HashMap::new();

    for line in reader.lines() {
        let ip_hostname = match line {
            Ok(v) => {
                let line_str = v.trim();
                let not_empty_line = !line_str.is_empty();
                let not_starting_with_hastag = !line_str.starts_with("#");
                if not_empty_line && not_starting_with_hastag {
                    line_str.to_string()
                } else {
                    continue;
                }
            }
            Err(_) => continue,
        };

        let ip_hostname_split: Vec<&str> = ip_hostname.split_whitespace().collect();
        if let Some((ip, hosts)) = ip_hostname_split.split_first() {
            for host in hosts {
                ip_host_map
                    .entry(ip.to_string())
                    .or_insert_with(Vec::new)
                    .push(host.to_string());
            }
        }
    }

    match is_tailscale_exists() {
        Ok(_) => {
            let tailscale_ip_host = list_tailscale_ip().unwrap();
            for tup in tailscale_ip_host {
                let (ip, hostname) = tup;
                ip_host_map
                    .entry(ip)
                    .or_insert_with(Vec::new)
                    .push(hostname);
            }
            write_file(&ip_host_map, &tmp_hosts)
        }
        Err(_) => Ok(()),
    }
}

fn is_tailscale_exists() -> io::Result<bool> {
    Command::new("tailscale")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
}

fn list_tailscale_ip() -> io::Result<Vec<(String, String)>> {
    let output = Command::new("tailscale").arg("status").output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to get Tailscale status",
        ));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut result = Vec::new();

    for line in output_str.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() >= 2 {
            result.push((parts[0].to_string(), parts[1].to_string()));
        }
    }

    if result.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No Tailscale IP found",
        ));
    }

    Ok(result)
}

fn write_file(
    ip_host_map: &HashMap<String, Vec<String>>,
    tmp_hosts: &PathBuf,
) -> std::io::Result<()> {
    let mut hosts_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(tmp_hosts)?;

    for (ip, hosts) in ip_host_map {
        let line = format!("{} {}", ip, hosts.join(" "));
        hosts_file.write_all(line.as_bytes())?;
        hosts_file.write_all(b"\n")?;
    }
    Ok(())
}
