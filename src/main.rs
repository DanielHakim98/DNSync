use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};

fn main() -> std::io::Result<()> {
    let filepath = "/etc/hosts";
    let file = File::open(filepath)?;
    let reader = io::BufReader::new(file);

    let tmp_dir = std::env::temp_dir();
    // let tmp_hosts_old = tmp_dir.join("hosts.old");
    let tmp_hosts = tmp_dir.join("hosts");

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
                    String::new()
                }
            }
            Err(_) => String::new(),
        };

        if ip_hostname.is_empty() {
            continue;
        }

        let ip_hostname_split: Vec<&str> = ip_hostname.split_whitespace().collect();
        if let Some((ip, hosts)) = ip_hostname_split.split_first() {
            // println!("ip: {}", ip);
            // print!("host: ");
            for host in hosts {
                // print!("{} ",host);
                ip_host_map
                    .entry(ip.to_string())
                    .or_insert_with(Vec::new)
                    .push(host.to_string());
            }
            // println!("");
        }
        // println!("{:#?}", ip_hostname_split)
    }

    // for (ip, hosts) in &ip_host_map {
    //     println!("IP: {}, Hosts: {:?}", ip, hosts);
    // }
    let mut hosts_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(tmp_hosts)?;

    for (ip, hosts) in &ip_host_map {
        hosts_file.write_all(ip.as_bytes())?;
        hosts_file.write_all(b" ")?;
        let mut host_iter = hosts.iter().peekable();
        while let Some(host) = host_iter.next() {
            hosts_file.write_all(host.as_bytes())?;
            if host_iter.peek().is_some() {
                hosts_file.write_all(b" ")?;
            }
        }
        hosts_file.write_all(b"\n")?;
    }

    Ok(())
}
