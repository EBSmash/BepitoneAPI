#[macro_use]
extern crate rocket;

use std::borrow::{Borrow, BorrowMut};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
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

static SCAN_COUNTERS: Lazy<Mutex<Vec<i32>>> = Lazy::new(|| Mutex::new(vec![-2, -1]));
static FAILED_SCANS: Lazy<Mutex<HashMap<String, i32>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
    writeln!(iterators, "{}", COUNTERS.lock().unwrap()[0].to_string()).expect("nothing at all");
    writeln!(iterators, "{}", COUNTERS.lock().unwrap()[1].to_string()).expect("nothing at all");

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

fn update_scan_fail(map: &mut HashMap<String, i32>) {
    fs::remove_file("static/scan_fail.bep").expect("ERRROR");
    File::create("static/scan_fail.bep").expect("MEOW");
    let mut file_out = OpenOptions::new()
        .write(true)
        .append(true)
        .open("static/scan_fail.bep")
        .unwrap();
    for (key, value) in &*map {
        writeln!(file_out, "{}", format!("{} {}", key.to_string(), value.to_string())).expect("DEWWWY");
    }
    map.clear();
}

fn update_scan() {
    fs::remove_file("static/scan_iterators.bep").expect("owo");
    File::create("static/scan_iterators.bep").expect("owozers");
    let mut scan_list: File = OpenOptions::new()
        .write(true)
        .read(false)
        .append(true)
        .open("static/scan_iterators.bep")
        .unwrap();
    for line in SCAN_COUNTERS.lock().unwrap().to_vec() {
        writeln!(scan_list, "{}", line).expect("rawr");
    }
}

#[get("/scanfail/<layer>/<user>")]
fn scan_fail(layer: i32, user: &str) {
    if FAILED_SCANS.lock().unwrap().contains_key(user) {
        FAILED_SCANS.lock().unwrap().remove(user);
    }
    FAILED_SCANS.lock().unwrap().insert(user.to_string(), layer);
    update_scan_fail(&mut(FAILED_SCANS.lock().unwrap()));
}

#[get("/scan/<last_scan>/<user>")]
fn scan(last_scan: i32, user: String) -> String {
    let mut assignment = "0".to_string();
    if FAILED_SCANS.lock().unwrap().contains_key(&user) {
        assignment = FAILED_SCANS.lock().unwrap().remove(&user).unwrap().to_string();
        update_scan_fail(&mut(FAILED_SCANS.lock().unwrap()));
    } else {
        if last_scan % 2 == 0 { // even
            SCAN_COUNTERS.lock().unwrap()[0] = SCAN_COUNTERS.lock().unwrap().get(0).unwrap() + 2;
            assignment = SCAN_COUNTERS.lock().unwrap().get(0).unwrap().to_string();
            update_scan()
        } else if last_scan % 2 != 1 { // odd
            SCAN_COUNTERS.lock().unwrap()[1] = SCAN_COUNTERS.lock().unwrap().get(1).unwrap() + 2;
            assignment = SCAN_COUNTERS.lock().unwrap().get(1).unwrap().to_string();
            update_scan()
        } else {
            return "DISABLE\n".to_string();
        }
    }
    let file = File::open(format!("static/partitions/{}", &*assignment));
    let reader = BufReader::new(file.unwrap());

    let mut lines = String::new();
    for line in reader.lines() {
        lines.push_str(&*format!("{}{}", &*line.unwrap(), "\n"));
    }
    println!("STARTING LAYER {}", assignment);
    return lines.to_string(); // todo don't send the whole file, completely unnecessary
}

#[get("/assign/<layer>/<user>/<restart>")]
fn assign(layer: i32, user: &str, restart: i8) -> String {
    let mut assignment = "0".to_string();
    if DISCONNECT_LAYERS.lock().unwrap().to_vec().contains(user.clone().to_string()) && restart == 1 {
        assignment = user.to_string();
    } else {
        DISCONNECT_LAYERS.lock().unwrap().retain(|value| *value != user);
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
    println!("{}", y);
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
        if let Err(e) = writeln!(file_out, "{}.failed", format!("{}", lines.get(0).unwrap())) {
            eprintln!("Couldn't write to file: {}", e);
        }

        if file_name.parse::<i32>().unwrap() % 2 == 0 { //even
            FAILED_LAYERS_EVEN.lock().unwrap().push(format!("{}.failed", file_name));
        } else {
            FAILED_LAYERS_ODD.lock().unwrap().push(format!("{}.failed", file_name));
        }

        for line_num in line_err - 1..lines.len() {
            let current = lines.get(line_num).unwrap();

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
        if !DISCONNECT_LAYERS.lock().unwrap().contains(name.clone()) {
            DISCONNECT_LAYERS.lock().unwrap().push(name.clone());
            update_failed();
        }
        for line_num in line_err - 1..lines.len() {
            let current = lines.get(line_num).unwrap();

            writeln!(file_out, "{}", current.to_string()).expect("failed to write");
            println!("{}", current);
        }
    }
}

#[get("/start")]
fn start() -> String {
    PLAYER_COUNT.lock().unwrap().push(0);
    let id = PLAYER_COUNT.lock().unwrap().len();
    // PLAYER_INDEX.lock().unwrap().insert(id as i32, id.to_string()); // to string is layer on first round
    id.to_string()
}
fn update_leaderboard(map: &mut HashMap<String, i32>) {
    fs::remove_file("static/leaderboard.bep").expect("ERRROR");
    File::create("static/leaderboard.bep").expect("MEOW");
    let mut file_out = OpenOptions::new()
        .write(true)
        .append(true)
        .open("static/leaderboard.bep")
        .unwrap();
    for (key, value) in &*map {
        writeln!(file_out, "{}", format!("{} {}", key.to_string(), value.to_string())).expect("DEWWWY");
    }
    map.clear();
}
#[get("/leaderboard/<user>/<iterator>")]
fn leaderboard(user:String, iterator:i32) {
    let mut leaderboard_buffer: HashMap<String, i32> = HashMap::new();
    let file = File::open("static/leaderboard.bep");

    let reader = BufReader::new(file.unwrap());
    for line in reader.lines() {
        leaderboard_buffer.insert(line.as_ref().unwrap().split(" ").collect::<Vec<&str>>()[0].to_string(), line.as_ref().unwrap().split(" ").collect::<Vec<&str>>()[1].parse::<i32>().expect("uwu"));
    }
    if leaderboard_buffer.contains_key(&user) {
        leaderboard_buffer.insert(user.clone(), leaderboard_buffer.get(user.as_str()).unwrap() + iterator);
    } else {
        leaderboard_buffer.insert(user, iterator);
    }
    update_leaderboard(&mut(leaderboard_buffer));
}

#[get("/end")]
fn end() {
    PLAYER_COUNT.lock().unwrap().remove(0);
}

#[launch]
fn rocket() -> _ { // idk but this fixed shit
    let file = File::open("static/scan_iterators.bep");
    let reader = BufReader::new(file.unwrap());
    let mut iter = 0;
    for line in reader.lines() {
        SCAN_COUNTERS.lock().unwrap()[iter] = line.unwrap().parse::<i32>().expect("meow");
        iter += 1;
    }

    let file = File::open("static/iterators.bep");

    let reader = BufReader::new(file.unwrap());
    let mut iter = 0;
    for line in reader.lines() {
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

    rocket::build().mount("/", routes![assign, start, end, fail_file_gen, leaderboard, scan, scan_fail])

}
