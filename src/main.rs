mod schema;

#[macro_use]
extern crate rocket;

use rocket::{Config, State};
use rocket::serde::{Serialize, json::Json};
// 1.3.1
use std::sync::Mutex;
use rocket::response::Responder;
use rusqlite::{Connection, named_params};

struct BepitoneState {
    db: Mutex<Connection>
}

fn next_layer(con: &Connection, is_even: bool) -> rusqlite::Result<i64> {
    // min is the default value and the value used to for odd/even
    let query = "
        INSERT INTO layers(layer) SELECT COALESCE(MAX(layer) + 2, :min) FROM layers WHERE (layer % 2) = :min
        RETURNING *;
    ";
    let mut statement = con.prepare(query)?;
    let arg = if is_even { 0 } else { 1 };
    let mut rows = statement.query((":min", arg))?;
    let row = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    return row.get(0);

}

fn get_layer_data(con: &Connection, layer: i64) -> rusqlite::Result<String> {
    let query = "SELECT serialized FROM partitions WHERE layer = :layer";
    let mut statement = con.prepare(query)?;
    let mut rows = statement.query((":layer", layer))?;
    let row = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    return row.get(0);
}

fn assign_to_layer(con: &Connection, user: &str, layer: i64) -> rusqlite::Result<()> {
    let query = "INSERT INTO assignments VALUES (:username, :layer, 0) ON CONFLICT REPLACE";
    let mut statement = con.prepare(query)?;
    return statement.execute(named_params! {
        ":username": user,
        ":layer": layer
    }).map(|_| ());
}

fn assign_to_next_layer(con: &mut Connection, user: &str, is_even: bool) -> rusqlite::Result<i64> {
    let tx = con.transaction()?;
    let layer = next_layer(&tx, is_even)?;
    assign_to_layer(&tx, user, layer)?;
    tx.commit()?;
    Ok(layer)
}

fn get_existing_assignment(con: &Connection, user: &str) -> rusqlite::Result<Option<i64>> {
    let query = "SELECT layer FROM assignments WHERE username = :username";
    let mut statement = con.prepare(query)?;
    let mut rows = statement.query((":username", user))?;
    if let Some(row) = rows.next()? {
        let layer = row.get(0)?;
        Ok(Some(layer))
    } else {
        Ok(None)
    }
}

#[derive(Responder)]
#[response(status = 500, content_type = "text/plain")]
struct SqlError {
    message: String
}
impl SqlError {
    fn new(err: rusqlite::Error) -> Self {
        SqlError { message: format!("Sqlite Error: {}", err.to_string()) }
    }
}

// restart means this user has just finished a layer
#[get("/assign/<user>/<even_or_odd>/<restart>")]
fn assign(state: &State<BepitoneState>, user: &str, even_or_odd: &str, restart: i32) -> Result<Option<String>, SqlError> {
    let is_even = match even_or_odd {
        "even" => true,
        "odd" => false,
        _ => return Ok(None) // 404
    };

    let mut con = state.db.lock().unwrap();

    let layer = if restart == 1 {
        assign_to_next_layer(&mut con, user, is_even)
    } else {
        let existing = get_existing_assignment(&con, user);
        match existing {
            Ok(Some(layer)) => Ok(layer),
            Ok(None) => assign_to_next_layer(&mut con, user, is_even),
            Err(err) => Err(err)
        }
    };
    let layer = layer.map_err(SqlError::new)?;
    let data = get_layer_data(&con, layer).map_err(SqlError::new)?;
    Ok(Some(data))
}

#[put("/finish/<user>")]
fn finish_layer(state: &State<BepitoneState>, user: &str) -> Result<(), SqlError> {
    let query = "DELETE FROM assignments WHERE username = ?";
    let con = state.db.lock().unwrap();
    let mut statement = con.prepare(query).map_err(SqlError::new)?;
    let result = statement.execute((1, user));
    result.map(|_| ()).map_err(SqlError::new)
}

#[put("/leaderboard/<user>/<value>")]
fn add_to_leaderboard(state: &State<BepitoneState>, user: String, value: i64) -> Result<(), SqlError> {
    let query = "
        INSERT INTO leaderboard (username, blocks_mined)
        VALUES (:username, :blocks_mined)
        ON CONFLICT (username)
        DO UPDATE
        SET blocks_mined = blocks_mined + :blocks_mined;
    ";
    let con = state.db.lock().unwrap();
    let mut statement = con.prepare(query).map_err(SqlError::new)?;
    let result = statement.execute(named_params! {
        ":username": user,
        ":blocks_mined": value
    });
    result.map(|_| ()).map_err(SqlError::new)
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct LeaderboardEntry {
    username: String,
    blocks_mined: i64
}

#[get("/leaderboard")]
fn leaderboard(state: &State<BepitoneState>) -> Result<Json<Vec<LeaderboardEntry>>, SqlError> {
    let query = "SELECT username, blocks_mined FROM leaderboard ORDER BY blocks_mined DESC";
    let con = state.db.lock().unwrap();
    let mut statement = con.prepare(query).map_err(SqlError::new)?;

    let rows = statement.query_map([], |row| {
        Ok(LeaderboardEntry {
            username: row.get(0)?,
            blocks_mined: row.get(1)?
        })
    }).map_err(SqlError::new)?;
    let mut entries = Vec::new();
    for entry in rows {
        entries.push(entry.map_err(SqlError::new)?);
    }

    return Ok(Json(entries));
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
