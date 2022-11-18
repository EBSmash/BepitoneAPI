#[macro_use]
extern crate rocket;

use std::fmt::format;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::ops::{Deref, Index};
use std::sync::atomic::AtomicU64;
use rocket::{Build, Rocket, State};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use rocket::form::validate::Contains;
use std::collections::HashMap;

static ROW_EVEN: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));
static ROW_ODD: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

static PLAYER_COUNT: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

static PLAYER_INDEX: Lazy<Mutex<Vec<Player>>> = Lazy::new(|| Mutex::new(vec![]));

static FAILED_LAYERS_EVEN: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));
static FAILED_LAYERS_ODD: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));


#[derive(Clone)]
struct Player {
    id: usize,
    column: i32,
    even: bool,
}

fn next_layer(isEven: bool) {
    // find next possible layer based on arguments and return the id of it (MUST CHECK IF THE THING HAS ALREADY BEEN DONE)
    // You can do this by having 2 global ints which increment by 2 each time they are selected (1 for even and 1 for odd)
}

#[get("/assign/<last_row>")]
fn assign(last_row: usize) -> String {
    let mut lines = String::new();
    let mut assignment = 69;

    
    if last_row % 2 == 0 { //even
        ROW_EVEN.lock().unwrap().push(0);
        ROW_EVEN.lock().unwrap().push(0);
        assignment = ROW_EVEN.lock().unwrap().len();

        if assignment % 2 == 0 {
            assignment += 1;
        }

        let file = File::open(format!("static/partitions/{}", &*assignment.to_string()));
        let reader = BufReader::new(file.unwrap());

        for line in reader.lines() {
            lines.push_str(&*format!("{}{}", &*line.unwrap(), "\n"));
        }
        println!("STARTING LAYER {}", assignment);

    } else { //odd
        ROW_ODD.lock().unwrap().push(0);
        ROW_ODD.lock().unwrap().push(0);
        assignment = ROW_ODD.lock().unwrap().len();

        if assignment % 2 != 0 {
            assignment += 1;
        }

        let file = File::open(format!("static/partitions/{}", &*assignment.to_string()));
        let reader = BufReader::new(file.unwrap());

        for line in reader.lines() {
            lines.push_str(&*format!("{}{}", &*line.unwrap(), "\n"));
        }
        println!("STARTING LAYER {}", assignment);

        return lines.to_string();

    }
    return lines.to_string();
}

#[get("/fail/<file_name>/<x>/<z>")]
fn fail_file_gen(file_name: &str, x: i32, z: i32) {
    let file = File::open(format!("static/partitions/{}", file_name));

    let reader = BufReader::new(file.unwrap());

    let mut lines = vec![];

    let mut line_err = 0;

    for mut line in reader.lines() {
        let formatted_line = format!("{} {}", x, z);
        let line = line.unwrap().clone();
        lines.push(line.clone());
        if line.clone().as_str().contains(formatted_line.as_str()) {
            line_err = lines.len()
        }
    }

    if file_name.contains(".failed") {
        fs::rename(file_name, file_name.replace(".failed", "")).expect("TODO: panic message");
        fs::remove_file(format!("static/partitions/{}.failed", file_name)).expect("MEOWWWWW");
    }

    let create_file = File::create(format!("static/partitions/{}.failed", file_name)).expect("errr");
    let mut file_out = OpenOptions::new() // TODO CREATE ZE FUCKING FILE BEFORE WE TRY TO WRITE TO IT
        .write(true)
        .append(true)
        .open(format!("static/partitions/{}.failed", file_name))
        .unwrap();

    if let Err(e) = write!(file_out, "{}.failed", format!("{}", lines.get(0).unwrap())) {
        eprintln!("Couldn't write to file: {}", e);
    }

    for lineNum in line_err - 1..lines.len() {
        let current = lines.get(lineNum).unwrap();

        writeln!(file_out, "{}", current.to_string()).expect("failed to write");
        println!("{}", current);
    }

    //TODO add file_name.failed to the QUEUE
    if file_name.parse::<i32>().unwrap() % 2 == 0 {
        FAILED_LAYERS_EVEN.lock().unwrap().push(format!("{}.failed", file_name));
    } else {
        FAILED_LAYERS_ODD.lock().unwrap().push(format!("{}.failed", file_name));
    }
    let mut failed_list = OpenOptions::new()
        .write(true)
        .read(false)
        .append(true)
        .open("static/failed_layers.bep")
        .unwrap();
    writeln!(failed_list, "{}.failed", file_name).expect("MEOWWWW");
    //TODO then copy the current QUEUE to the queue log text file
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
    let id = PLAYER_COUNT.lock().unwrap().len();
    id.to_string()
}

#[get("/end")]
fn end() {
    PLAYER_COUNT.lock().unwrap().remove(0);
}

#[launch]
fn rocket() -> _ { // idk but this fixed shit
    rocket::build().mount("/", routes![assign, broken,start, end, fail_file_gen])
}