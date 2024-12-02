use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};

fn main() ->std::io::Result<()>{
    let filepath = "/etc/hosts";
    let file = File::open(filepath)?;
    let reader = io::BufReader::new(file);

    let mut ip_host_map: HashMap<String, Vec<String>> = HashMap::new();

    for line in reader.lines(){
        let ip_hostname = match line{
            Ok(v)=>{
                let line_str =v.trim();
                let not_empty_line = !line_str.is_empty();
                let not_starting_with_hastag = !line_str.starts_with("#");
                if not_empty_line && not_starting_with_hastag{
                    line_str.to_string()
                } else{
                    String::new()
                }
            }
            Err(_)=>{
                String::new()
            }
        };

        if ip_hostname.is_empty() {
            continue
        }

        let ip_hostname_split: Vec<&str> = ip_hostname.split_whitespace().collect();
        if let Some((ip, hosts))= ip_hostname_split.split_first(){
            // println!("ip: {}", ip);
            // print!("host: ");
            for host in hosts{
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
    for (ip, hosts) in &ip_host_map {
        println!("IP: {}, Hosts: {:?}", ip, hosts);
    }
    Ok(())
}
