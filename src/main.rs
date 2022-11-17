#[macro_use]
extern crate rocket;

use std::fmt::format;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::ops::Deref;
use std::sync::atomic::AtomicU64;
use rocket::{Build, Rocket, State};
use once_cell::sync::Lazy;
// 1.3.1
use std::sync::Mutex;
use rocket::form::validate::Contains;

static ROW: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

static PLAYER_COUNT: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

fn add() {
    ROW.lock().unwrap().push(0);
}


#[get("/assign/<id>")]
fn assign(id: i32) -> String {
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

struct Coord {
    x: i32,
    z: i32,
}

#[get("/fail/<file_name>/<x>/<z>")]
fn fail_file_gen(file_name: &str, x:i32,z:i32) {
    let file = File::open(file_name);

    let reader = BufReader::new(file.unwrap());

    let mut lines = Vec::new();

    let mut line_err = 0;

    for line in reader.lines() {
        lines.push(line.unwrap());
        if line.unwrap().contains(format!("{} {}", x, z)) {
            line_err = lines.len()
        }
    }

    if file_name.contains(".failed") {
        fs::rename(file_name, file_name.replace(".failed", "")).expect("TODO: panic message");
    }

    let file_out = OpenOptions::new()
        .write(true)
        .append(true)
        .open(format!("static/{}.failed", file_name))
        .unwrap();

    if let Err(e) = write!(file_out, "{}", format!("{}", lines.get(0).unwrap())) {
        eprintln!("Couldn't write to file: {}", e);
    }

    for lineNum in line_err..lines.len() {
        let current = lines.get(lineNum).unwrap();

        writeln!(file_out, "{}", current.to_string()).expect("failed to write");
        println!("{}", current);
    }

    writeln!(file_out, "{}", lines.get(lines.len()).unwrap()).expect("failed to write");
}


#[get("/broken/<id>/<x>/<z>")]
fn broken(id: &str, x: i32, z: i32) {
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open("static/broken.txt")
        .unwrap();

    if let Err(e) = writeln!(file, "{}", format!("{} {},{}", id, x, z)) {
        eprintln!("Couldn't write to file: {}", e);
    }
}

#[get("/start")]
fn start() -> String {
    PLAYER_COUNT.lock().unwrap().push(0);
    ROW.lock().unwrap().len().to_string()
}

#[get("/end")]
fn end() {
    PLAYER_COUNT.lock().unwrap().remove(0);
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![assign, broken, end, fail_file_gen])
}