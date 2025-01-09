use clap::Parser;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, ErrorKind, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short, long, default_value = "/etc/hosts")]
    source: PathBuf,

    #[arg(short = 'p', long, default_value = "/tmp/hosts.temp")]
    temp: PathBuf,

    #[arg(short, long, default_value = "/etc/hosts.old")]
    backup: PathBuf,

    #[arg(short = 't', long, default_value = "/etc/hosts")]
    target: PathBuf,
}

fn main() {
    let args = Cli::parse();
    let source_filepath = args.source;
    let file = File::open(&source_filepath).unwrap_or_else(|err| match err.kind() {
        ErrorKind::NotFound => {
            eprintln!(
                "Error: '/etc/hosts': No such file or directory. Please ensure '{}' exists",
                &source_filepath.to_str().unwrap()
            );
            std::process::exit(1);
        }
        _ => {
            eprintln!(
                "Error: '{}': {}",
                source_filepath.to_str().unwrap(),
                err.to_string()
            );
            std::process::exit(1)
        }
    });

    // TODO: Should skip reading if the source_path is just created
    let reader = io::BufReader::new(file);
    let mut ip_host_map: HashMap<String, Vec<String>> = HashMap::new();
    for line in reader.lines() {
        let ip_hostname = match line {
            Ok(v) => extract_hosts(&v),
            Err(_) => continue,
        };

        add_hosts_to_map(&ip_hostname, &mut ip_host_map);
    }

    // if 'filename' is included in --target, then use that
    // else, assume 'filename' is implicitly taken from --source
    let temp = args.temp;
    let temp_filepath = if let Some(_) = temp.file_name() {
        temp
    } else {
        temp.join(source_filepath.file_name().unwrap())
    };

    let tailscale_ip_hosts = match is_tailscale_exists() {
        Ok(true) => list_tailscale_ip().unwrap_or_else(|error| {
            eprintln!("Error: 'tailscale status' :{}", error.to_string());
            std::process::exit(1)
        }),
        Ok(false) => {
            eprintln!("Error: 'tailscale --version': tailscale installed but not working");
            std::process::exit(1)
        }
        Err(error) => {
            eprintln!("Error: 'tailscale status': {}", error.to_string());
            std::process::exit(1)
        }
    };

    for tup in tailscale_ip_hosts {
        let (ip, hostname_tailscale) = tup;
        let hostname_list = ip_host_map.entry(ip).or_insert_with(Vec::new);
        let not_contain_hostname_tailscale = !hostname_list.contains(&hostname_tailscale);

        // prevent duplicates in hostname_list in particular ip
        if not_contain_hostname_tailscale {
            hostname_list.push(hostname_tailscale);
        }
    }

    write_file(&mut ip_host_map, &temp_filepath).unwrap_or_else(|err| {
        match err.kind() {
            std::io::ErrorKind::NotFound => {
                eprintln!(
                    "Error: '{}' does not exist. Please ensure the file path is correct.",
                    temp_filepath.to_string_lossy()
                );
            }
            _ => {
                eprintln!("Error: '{}': {}", temp_filepath.to_string_lossy(), err);
            }
        }
        std::process::exit(1);
    });

    create_backup(&source_filepath)
        .unwrap_or_else(|err| eprintln!("Error: '{}': {}", source_filepath.to_string_lossy(), err));

    let target: PathBuf = args.target;
    let target_filepath = if let Some(_) = target.file_name() {
        target
    } else {
        target.join(source_filepath.file_name().unwrap())
    };

    replace_source_file(&target_filepath, &temp_filepath)
        .unwrap_or_else(|err| eprintln!("Error: '{}': {}", temp_filepath.to_string_lossy(), err));
}

fn extract_hosts(value: &str) -> String {
    let line_str = value.trim();
    let not_empty_line = !line_str.is_empty();
    let not_starting_with_hastag = !line_str.starts_with("#");
    if not_empty_line && not_starting_with_hastag {
        line_str.to_string()
    } else {
        "".to_string()
    }
}

fn add_hosts_to_map(ip_hostname: &str, ip_host_map: &mut HashMap<String, Vec<String>>) {
    let ip_hostname_split: Vec<&str> = ip_hostname.split_whitespace().collect();
    if let Some((ip, hostnames)) = ip_hostname_split.split_first() {
        for name in hostnames {
            ip_host_map
                .entry(ip.to_string())
                .or_insert_with(Vec::new)
                .push(name.to_string());
        }
    }
}

fn is_tailscale_exists() -> io::Result<bool> {
    match Command::new("tailscale").arg("--version").output() {
        Ok(output) => Ok(output.status.success()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Tailscale binary not found",
        )),
        Err(e) => Err(e),
    }
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

    for line in output_str.lines().map(str::trim).filter(|l| !l.is_empty()) {
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
    ip_host_map: &mut HashMap<String, Vec<String>>,
    target_filepath: &PathBuf,
) -> io::Result<()> {
    let mut hosts_file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(target_filepath)?;

    if ip_host_map.is_empty() {
        eprintln!("Warning: ip_host_map is empty, only default IPs will be written.");
    }

    let default_ip = vec!["127.0.0.1".to_string(), "::1".to_string()];
    let max_ip_len = ip_host_map
        .keys()
        .chain(default_ip.iter())
        .map(|ip| ip.len())
        .max()
        .unwrap_or(0);

    let mut write_line = |ip: &str, hosts: &Vec<String>| -> std::io::Result<()> {
        let padded_ip = format!("{:width$}", ip, width = max_ip_len);
        let line = format!("{} {}", padded_ip, hosts.join(" "));
        hosts_file.write_all(line.as_bytes())?;
        hosts_file.write_all(b"\n")?;
        Ok(())
    };

    // Ensure placement of default loopback address is always on the top
    // Same as above but specifically for ipv6
    for ip in default_ip {
        let localhost_hosts = ip_host_map
            .remove(&ip)
            .unwrap_or_else(|| vec!["localhost".to_string()]);
        write_line(&ip, &localhost_hosts)?;
    }

    for (ip, hosts) in ip_host_map {
        write_line(ip, hosts)?;
    }

    Ok(())
}

fn create_backup(source_filepath: &PathBuf) -> io::Result<()> {
    let backup_filepath = source_filepath.with_extension("old");
    std::fs::copy(source_filepath, backup_filepath)?;
    Ok(())
}

fn replace_source_file(target_filepath: &PathBuf, source_filepath: &PathBuf) -> io::Result<()> {
    println!(
        "target: {:?}, source: {:?}",
        target_filepath, source_filepath
    );
    std::fs::copy(source_filepath, target_filepath)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_hosts_ok() {
        let input =
            "127.0.0.1   localhost localhost.localdomain localhost4 localhost4.localdomain4";
        let result = extract_hosts(input);
        let expected = String::from(
            "127.0.0.1   localhost localhost.localdomain localhost4 localhost4.localdomain4",
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_extract_hosts_commented() {
        let input = "# 192.168.1.10 foo.example.org foo";
        let result = extract_hosts(input);
        let expected = String::from("");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_extract_hosts_empty_line() {
        let input = "";
        let result = extract_hosts(input);
        let expected = String::from("");
        assert_eq!(result, expected);
    }
}
