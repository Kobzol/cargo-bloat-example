use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct S {
    a: u32
}

fn main() {
    let s: S = serde_json::from_slice(&[]).unwrap();
    ureq::get("http://seznam.cz");
    println!("hello");
}
