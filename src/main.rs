mod schema;

#[macro_use]
extern crate rocket;

use rocket::{Config, State};
use rocket::serde::{Serialize, json::Json};
// 1.3.1
use std::sync::Mutex;
use rusqlite::{Connection, named_params};

struct BepitoneState {
    db: Mutex<Connection>
}

fn next_layer(con: &Connection, is_even: bool) -> i64 {
    // min is the default value and the value used to for odd/even
    let query = "
        INSERT INTO layers(layer) SELECT COALESCE(MAX(layer) + 2, :min) FROM layers WHERE (layer % 2) = :min
        RETURNING *;
    ";
    let mut statement = con.prepare(query).unwrap();
    let arg = if is_even { 0 } else { 1 };
    let mut rows = statement.query((":min", arg)).unwrap();
    let row = rows.next().unwrap().unwrap();
    return row.get_unwrap(0);

}

fn get_layer_data(con: &Connection, layer: i64) -> String {
    let query = "SELECT serialized FROM partitions WHERE layer = :layer";
    let mut statement = con.prepare(query).unwrap();
    let mut rows = statement.query((":layer", layer)).unwrap();
    let row = rows.next().unwrap().unwrap();
    return row.get_unwrap(0);
}

fn assign_to_layer(con: &Connection, user: &str, layer: i64) {
    let query = "INSERT INTO assignments VALUES (:username, :layer, 0) ON CONFLICT REPLACE";
    let mut statement = con.prepare(query).unwrap();
    statement.execute(named_params! {
        ":username": user,
        ":layer": layer
    }).unwrap();
}

fn assign_to_next_layer(con: &mut Connection, user: &str, is_even: bool) -> i64 {
    let tx = con.transaction().unwrap();
    let layer = next_layer(&tx, is_even);
    assign_to_layer(&tx, user, layer);
    tx.commit().unwrap();
    layer
}

fn get_existing_assignment(con: &Connection, user: &str) -> Option<i64> {
    let query = "SELECT layer FROM assignments WHERE username = :username";
    let mut statement = con.prepare(query).unwrap();
    let mut rows = statement.query((":username", user)).unwrap();
    if let Some(row) = rows.next().unwrap() {
        let layer = row.get_unwrap(0);
        Some(layer)
    } else {
        None
    }
}

// restart means this user has just finished a layer
#[get("/assign/<user>/<even_or_odd>/<restart>")]
fn assign(state: &State<BepitoneState>, user: &str, even_or_odd: &str, restart: i32) -> Option<String> {
    let is_even = match even_or_odd {
        "even" => true,
        "odd" => false,
        _ => return None // 404
    };

    let mut con = state.db.lock().unwrap();

    let layer = if restart == 1 {
        assign_to_next_layer(&mut con, user, is_even)
    } else {
        let existing = get_existing_assignment(&con, user);
        let layer = existing.unwrap_or_else(|| assign_to_next_layer(&mut con, user, is_even));
        layer
    };

    return Some(get_layer_data(&con, layer));
}

#[put("/finish/<user>")]
fn finish_layer(state: &State<BepitoneState>, user: &str) {
    let query = "DELETE FROM assignments WHERE username = ?";
    let con = state.db.lock().unwrap();
    let mut statement = con.prepare(query).unwrap();
    let result = statement.execute((1, user));
    if !result.is_ok() {
        panic!()
    }
}

#[put("/leaderboard/<user>/<value>")]
fn add_to_leaderboard(state: &State<BepitoneState>, user: String, value: i64) {
    let query = "
        INSERT INTO leaderboard (username, blocks_mined)
        VALUES (:username, :blocks_mined)
        ON CONFLICT (username)
        DO UPDATE
        SET blocks_mined = blocks_mined + :blocks_mined;
    ";
    let con = state.db.lock().unwrap();
    let mut statement = con.prepare(query).unwrap();
    let result = statement.execute(named_params! {
        ":username": user,
        ":blocks_mined": value
    });
    if !result.is_ok() {
        panic!();
    }
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct LeaderboardEntry {
    username: String,
    blocks_mined: i64
}

#[get("/leaderboard")]
fn leaderboard(state: &State<BepitoneState>) -> Json<Vec<LeaderboardEntry>> {
    let query = "SELECT username, blocks_mined FROM leaderboard ORDER BY blocks_mined DESC";
    let con = state.db.lock().unwrap();
    let mut statement = con.prepare(query).unwrap();

    let vec = statement.query_map([], |row| {
        Ok(LeaderboardEntry {
            username: row.get(0)?,
            blocks_mined: row.get(1)?
        })
    })
    .unwrap()
    .map(|row| row.unwrap())
    .collect();

    return Json(vec);
}

#[launch]
fn rocket() -> _ { // idk but this fixed shit
    let connection = Connection::open("bepitone.db").expect("Failed to open sqlite database (bepitone.db)");

    schema::apply_schema(&connection);

    let rocket = rocket::build()
        .manage(BepitoneState {
            db: Mutex::new(connection)
        });
    let figment = rocket.figment().clone()
        .merge((Config::PORT, 6969));
        //.merge((Config::ADDRESS, "0.0.0.0"));
    rocket.configure(figment)
        .mount("/", routes![assign, finish_layer, leaderboard, add_to_leaderboard])
}
