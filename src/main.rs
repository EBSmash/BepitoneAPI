#[macro_use] extern crate rocket;

use std::fmt::format;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
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

#[get("/broken/<id>/<x>/<z>")]
fn broken(id:&str, x:i32, z:i32) {
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open("static/broken.txt")
        .unwrap();

    if let Err(e) = writeln!(file, "{}", format!("{} {},{}", id, x,z)) {
        eprintln!("Couldn't write to file: {}", e);
    }
}


#[get("/failed/<id>/<layer_num>")]
fn failed(id:&str, layer_num:i32) {
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open("static/failed.txt")
        .unwrap();

    if let Err(e) = writeln!(file, "{}", layer_num.to_string())) {
        eprintln!("Couldn't write to file: {}", e);
    }
}


#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![assign, broken,failed])
}