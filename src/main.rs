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

static DISCONNECT_LAYERS: Lazy<Mutex<Vec<String>>> = Lazy::new(||Mutex::new(vec![]));

static COUNTERS: Lazy<Mutex<Vec<i32>>> = Lazy::new(||Mutex::new(vec![-2,-1]));

static PLAYER_COUNT: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| Mutex::new(vec![]));

static FAILED_LAYERS_EVEN: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));
static FAILED_LAYERS_ODD: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(vec![]));

fn next_layer(isEven: bool) -> i32 {
    let mut out:i32;
    if isEven {
        COUNTERS.lock().unwrap()[0]+=2;
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
    writeln!(iterators,"{}", out.to_string()).expect("dewyyyy");
    return out;
}

fn update_failed () { // todo

}

#[get("/assign/<layer>/<name>")]
fn assign(layer: i32, name: String) -> String {
    let mut assignment = "0".to_string();
    if DISCONNECT_LAYERS.lock().unwrap().contains(name.clone()) {
        // DISCONNECT_LAYERS.lock().unwrap().remove(DISCONNECT_LAYERS.lock().unwrap().retain(|value| *value != name)); // maybe
        DISCONNECT_LAYERS.lock().unwrap().retain(|value| *value != name);
        assignment = format!("{}.{}",layer.to_string(),name);
    } else {
        if layer % 2 == 0 { // EVEN
            if FAILED_LAYERS_EVEN.lock().unwrap().len() != 0 {
                assignment = FAILED_LAYERS_EVEN.lock().unwrap().get(0).unwrap().to_string();
                FAILED_LAYERS_EVEN.lock().unwrap().remove(0);
            } else {// assign odd
                assignment = next_layer(false).to_string();
            }
        } else { // ODD
            if FAILED_LAYERS_ODD.lock().unwrap().len() != 0 {
                assignment = FAILED_LAYERS_ODD.lock().unwrap().get(0).unwrap().to_string();
                FAILED_LAYERS_ODD.lock().unwrap().remove(0);
                //TODO update the FAILED_LAYERS text file (maybe make function to do this?)
            } else { //assign even
                assignment = next_layer(true).to_string();
            }
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
    if file_name.contains(&format!(".{}", name)) {
        fs::rename(file_name, file_name.replace(&format!(".{}", name), "")).expect("MEOW");
        fs::remove_file(format!("static/partitions/{}.{}", file_name, name)).expect("UWU");
    }
    if file_name.contains(".failed") {
        fs::rename(file_name, file_name.replace(".failed", "")).expect("TODO: panic message");
        fs::remove_file(format!("static/partitions/{}.failed", file_name)).expect("MEOWWWWW");
    }
    if y != 256 {
        let create_file = File::create(format!("static/partitions/{}.failed", file_name)).expect("errr");
        let mut file_out = OpenOptions::new()
            .write(true)
            .append(true)
            .open(format!("static/partitions/{}.failed", file_name))
            .unwrap();
        if let Err(e) = write!(file_out, "{}.failed", format!("{}", lines.get(0).unwrap())) {
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
    } else {
        File::create(format!("static/partitions/{}.{}", file_name, name)).expect("errr");
        let mut file_out = OpenOptions::new()
            .write(true)
            .append(true)
            .open(format!("static/partitions/{}.{}", file_name, name))
            .unwrap();
        if let Err(e) = write!(file_out, "{}", format!("{}.{}", file_name, name)) {
            eprintln!("Couldn't write to file: {}", e);
        }
        DISCONNECT_LAYERS.lock().unwrap().push(name.clone());
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
        writeln!(failed_list, "{}.{}", file_name, name).expect("DEWYYYY");
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
    rocket::build().mount("/", routes![assign, start, end, fail_file_gen])
}
