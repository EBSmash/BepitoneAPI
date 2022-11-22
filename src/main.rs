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
// 1.3.1
use std::sync::Mutex;
use rocket::form::validate::Contains;
use std::collections::HashMap;

static COUNTERS: Lazy<Mutex<Vec<i32>>> = Lazy::new(|| Mutex::new(vec![-2, -1]));

static PLAYER_COUNT: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

static FAILED_LAYERS_EVEN: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));
static FAILED_LAYERS_ODD: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));

static DISCONNECT_LAYERS: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));

fn next_layer(is_even: bool) -> i32 {
    let mut out: i32;
    if is_even {
        COUNTERS.lock().unwrap()[0] += 2;
        out = COUNTERS.lock().unwrap()[0];
    } else {
        COUNTERS.lock().unwrap()[1] += 2;
        out = COUNTERS.lock().unwrap()[1];
    }
    fs::remove_file("static/iterators.bep").expect("haram");
    File::create(format!("static/iterators.bep")).expect("halal");
    let mut iterators = OpenOptions::new()
        .write(true)
        .read(false)
        .append(true)
        .open("static/iterators.bep")
        .unwrap();
    writeln!(iterators, "{}", COUNTERS.lock().unwrap()[0].to_string());
    writeln!(iterators, "{}", COUNTERS.lock().unwrap()[1].to_string());

    return out;
}

fn update_failed() {
    fs::remove_file("static/failed_layers.bep").expect("reeeeee");
    File::create("static/failed_layers.bep").expect("meow");
    let mut failed_list = OpenOptions::new()
        .write(true)
        .read(false)
        .append(true)
        .open("static/failed_layers.bep")
        .unwrap();
    for line in FAILED_LAYERS_EVEN.lock().unwrap().to_vec() {
        writeln!(failed_list,"{}", line).expect("uwu");
    }
    for line in FAILED_LAYERS_ODD.lock().unwrap().to_vec() {
        writeln!(failed_list,"{}", line).expect("OWO");
    }
    for line in DISCONNECT_LAYERS.lock().unwrap().to_vec() {
        writeln!(failed_list,"{}",line).expect("awa");
    }
}

#[get("/assign/<layer>/<user>")]
fn assign(layer: i32, user: &str) -> String {
    let mut assignment = "0".to_string();
    if DISCONNECT_LAYERS.lock().unwrap().to_vec().contains(user.clone().to_string()) {
        DISCONNECT_LAYERS.lock().unwrap().retain(|value| *value != user);
        assignment = user.to_string();
    } else {
        if layer % 2 == 0 { // EVEN
            if FAILED_LAYERS_EVEN.lock().unwrap().len() != 0 {
                assignment = FAILED_LAYERS_EVEN.lock().unwrap().get(0).unwrap().to_string();
                FAILED_LAYERS_EVEN.lock().unwrap().remove(0);
                // fs::remove_file(format!("{}.failed", assignment));
            } else {// assign odd
                assignment = next_layer(false).to_string();
            }
        } else { // ODD
            if FAILED_LAYERS_ODD.lock().unwrap().len() != 0 {
                assignment = FAILED_LAYERS_ODD.lock().unwrap().get(0).unwrap().to_string();
                FAILED_LAYERS_ODD.lock().unwrap().remove(0);
                // fs::remove_file(format!("{}.failed", assignment));
                //TODO update the FAILED_LAYERS text file (maybe make function to do this?)
            } else { //assign even
                assignment = next_layer(true).to_string();
            }
        }
    }
    update_failed();

    let file = File::open(format!("static/partitions/{}", &*assignment));
    let reader = BufReader::new(file.unwrap());

    let mut lines = String::new();
    for line in reader.lines() {
        lines.push_str(&*format!("{}{}", &*line.unwrap(), "\n"));
    }
    println!("STARTING LAYER {}", assignment);
    return lines.to_string();
}

#[get("/fail/<file_name>/<x>/<y>/<z>/<name>")]
fn fail_file_gen(file_name: &str, x: i32, y:i32, z: i32, name: String) {
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
    if y != 256 {
        File::create(format!("static/partitions/{}.failed", file_name)).expect("errr");
        let mut file_out = OpenOptions::new()
            .write(true)
            .append(true)
            .open(format!("static/partitions/{}.failed", file_name))
            .unwrap();
        if let Err(e) = write!(file_out, "{}", format!("{}", lines.get(0).unwrap())) {
            eprintln!("Couldn't write to file: {}", e);
        }

        if file_name.parse::<i32>().unwrap() % 2 == 0 { //even
            FAILED_LAYERS_EVEN.lock().unwrap().push(format!("{}.failed", file_name));
        } else {
            FAILED_LAYERS_ODD.lock().unwrap().push(format!("{}.failed", file_name));
        }

        for lineNum in line_err - 1..lines.len() {
            let current = lines.get(lineNum).unwrap();

            writeln!(file_out, "{}", current.to_string()).expect("failed to write");
            println!("{}", current);
        }
        update_failed()
        //TODO then copy the current QUEUE to the queue log text file
    } else {
        File::create(format!("static/partitions/{}", name)).expect("errr");
        let mut file_out = OpenOptions::new()
            .write(true)
            .append(true)
            .open(format!("static/partitions/{}", name))
            .unwrap();
        if let Err(e) = write!(file_out, "{}", format!("{}\n", file_name)) {
            eprintln!("Couldn't write to file: {}", e);
        }
        DISCONNECT_LAYERS.lock().unwrap().push(name.clone());
        for lineNum in line_err - 1..lines.len() {
            let current = lines.get(lineNum).unwrap();

            writeln!(file_out, "{}", current.to_string()).expect("failed to write");
            println!("{}", current);
        }
        update_failed();
    }
}

#[get("/start")]
fn start() -> String {
    PLAYER_COUNT.lock().unwrap().push(0);
    let id = PLAYER_COUNT.lock().unwrap().len();
    // PLAYER_INDEX.lock().unwrap().insert(id as i32, id.to_string()); // to string is layer on first round
    id.to_string()
}

#[get("/end")]
fn end() {
    PLAYER_COUNT.lock().unwrap().remove(0);
}

#[launch]
fn rocket() -> _ { // idk but this fixed shit

    let file = File::open("static/iterators.bep");

    let reader = BufReader::new(file.unwrap());
    let mut iter = 0;
    for line in reader.lines() {
        // if line.unwrap().parse::<i32>() {
        //     for _ in 0..line.unwrap().parse::<i32>() {
        //         COUNTERS.lock().unwrap().push(0)
        //     }
        // }
        COUNTERS.lock().unwrap()[iter] = line.unwrap().parse::<i32>().expect("owo");
        iter += 1;
    }
    let file = File::open("static/failed_layers.bep");
    let reader = BufReader::new(file.unwrap());
    for line in reader.lines() {
        if line.as_ref().unwrap().contains(".failed") {
            if line.as_ref().unwrap().split(".").collect::<Vec<&str>>()[0].parse::<i32>().expect("meow") % 2 == 0 {
                FAILED_LAYERS_EVEN.lock().unwrap().push(line.unwrap().to_string());
            } else {
                FAILED_LAYERS_ODD.lock().unwrap().push(line.unwrap().to_string());
            }
        } else {
            DISCONNECT_LAYERS.lock().unwrap().push(line.unwrap().to_string());
        }
    }

    rocket::build().mount("/", routes![assign, start, end, fail_file_gen])

}
