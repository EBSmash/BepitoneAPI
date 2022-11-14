#[macro_use] extern crate rocket;

use std::fmt::format;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::sync::atomic::AtomicU64;
use rocket::{Build, Rocket, State};
use once_cell::sync::Lazy; // 1.3.1
use std::sync::Mutex;

static ROW: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

fn add() {
    ROW.lock().unwrap().push(0);
}


#[get("/assign")]
fn assign() -> String {

    let file = File::open(format!("static/partitions/{}", ROW.lock().unwrap().len()));
    let reader = BufReader::new(file.unwrap());

    let mut lines = String::new();

    for line in reader.lines() {
        lines.push_str(&*format!("{}{}", &*line.unwrap(), "\n"));
    }

    add();

    println!("STARTING LAYER {}", ROW.lock().unwrap().len());

    return lines.to_string();
}


#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![assign])
}