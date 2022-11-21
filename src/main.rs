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


static DC_LAYERS: Lazy<Mutex<HashMap<String, i32>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn next_layer(is_even: bool) -> i32 {
    let mut out: i32;
    if is_even {
        COUNTERS.lock().unwrap()[0] += 2;
        out = COUNTERS.lock().unwrap()[0];
    } else {
        COUNTERS.lock().unwrap()[1] += 2;
        out = COUNTERS.lock().unwrap()[1];
    }
    let make_file = File::create(format!("static/iterators.bep"));
    let mut iterators = OpenOptions::new()
        .write(true)
        .read(false)
        .append(false)
        .open("static/iterators.bep")
        .unwrap();
    writeln!(iterators, "{}", out.to_string()).expect("dewyyyy");
    return out;
}

fn update_failed() {}

#[get("/assign/<layer>/<user>")]
fn assign(layer: i32, user: &str) -> String {
    let mut assignment = "0".to_string();


    if layer % 2 == 0 { // EVEN
        if FAILED_LAYERS_EVEN.lock().unwrap().len() != 0 {
            assignment = FAILED_LAYERS_EVEN.lock().unwrap().get(0).unwrap().to_string();
            FAILED_LAYERS_EVEN.lock().unwrap().remove(0);
            fs::remove_file(format!("{}.failed", assignment));
        } else {// assign odd
            assignment = next_layer(false).to_string();
        }
    } else { // ODD
        if FAILED_LAYERS_ODD.lock().unwrap().len() != 0 {
            assignment = FAILED_LAYERS_ODD.lock().unwrap().get(0).unwrap().to_string();
            FAILED_LAYERS_ODD.lock().unwrap().remove(0);
            fs::remove_file(format!("{}.failed", assignment));
            //TODO update the FAILED_LAYERS text file (maybe make function to do this?)
        } else { //assign even
            assignment = next_layer(true).to_string();
        }
    }

    if DC_LAYERS.lock().unwrap().len() != 0 {
        if DC_LAYERS.lock().unwrap().contains_key(user) {
            assignment = DC_LAYERS.lock().unwrap().get(user).unwrap().to_string();
            fs::remove_file(format!("{}.disconnected", assignment));
            DC_LAYERS.lock().unwrap().remove(user);
        }
    }


    let file = File::open(format!("static/partitions/{}", &*assignment));
    let reader = BufReader::new(file.unwrap());

    let mut lines = String::new();
    for line in reader.lines() {
        lines.push_str(&*format!("{}{}", &*line.unwrap(), "\n"));
    }
    println!("STARTING LAYER {}", assignment);
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
    let _create_file = File::create(format!("static/partitions/{}.failed", file_name)).expect("errr");
    let mut file_out = OpenOptions::new()
        .write(true)
        .append(true)
        .open(format!("static/partitions/{}.failed", file_name))
        .unwrap();
    if let Err(e) = write!(file_out, "{}.failed\n", format!("{}", lines.get(0).unwrap())) {
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

    let mut failed_list = OpenOptions::new()
        .write(true)
        .read(false)
        .append(true)
        .open("static/failed_layers.bep")
        .unwrap();
    writeln!(failed_list, "{}.failed", file_name).expect("MEOWWWW");
    //TODO then copy the current QUEUE to the queue log text file
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


#[get("/dc/<file_name>/<x>/<z>/<username>")]
fn disconnect_file_gen(file_name: &str, x: i32, z: i32, username: &str) {
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
        fs::rename(file_name, file_name.replace(".disconnected", "")).expect("TODO: panic message");
        fs::remove_file(format!("static/partitions/{}.disconnected", file_name)).expect("MEOWWWWW");
    }
    let _create_file = File::create(format!("static/partitions/{}.disconnected", file_name)).expect("errr");
    let mut file_out = OpenOptions::new()
        .write(true)
        .append(true)
        .open(format!("static/partitions/{}.disconnected", file_name))
        .unwrap();
    if let Err(e) = write!(file_out, "{}.disconnected.{}\n", format!("{}", lines.get(0).unwrap()), username) {
        eprintln!("Couldn't write to file: {}", e);
    }

    DC_LAYERS.lock().unwrap().insert(username.to_string(), file_name.parse::<i32>().unwrap());

    if lines.len() > 0 {
        for lineNum in line_err..lines.len() {
            let current = lines.get(lineNum).unwrap();

            writeln!(file_out, "{}", current.to_string()).expect("failed to write");
            println!("{}", current);
        }
    }
}


#[launch]
fn rocket() -> _ { // idk but this fixed shit

    let file = File::open("static/iterators.bep");

    let reader = BufReader::new(file.unwrap());

    for line in reader.lines() {
        if line.as_ref().unwrap().parse::<i32>().unwrap() > 0 {
            for _ in 0..line.as_ref().clone().unwrap().parse::<i32>().unwrap() {
                COUNTERS.lock().unwrap()[0] +=1;
                COUNTERS.lock().unwrap()[1] +=1;
            }

            COUNTERS.lock().unwrap()[0] -=1;
            COUNTERS.lock().unwrap()[1] -=2;

            println!("{}", COUNTERS.lock().unwrap()[0]);
            println!("{}", COUNTERS.lock().unwrap()[1])
        }
    }

    rocket::build().mount("/", routes![assign, start, end, fail_file_gen, disconnect_file_gen])
}
