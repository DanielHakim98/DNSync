use std::fs;

fn main() {
    let filepath = "/etc/hosts";
    let hosts = fs::read_to_string(filepath)
    .expect(&format!("can't read {filepath}")[..]);
    println!("{hosts}")
}
