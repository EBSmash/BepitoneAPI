mod schema;

#[macro_use]
extern crate rocket;

use rocket::{Config, State};
use rocket::serde::{Serialize, json::Json};
// 1.3.1
use std::sync::Mutex;
use rocket::response::Responder;
use rusqlite::{Connection, named_params, OptionalExtension, params};

fn next_layer(con: &Connection, is_even: bool) -> rusqlite::Result<i64> {
    // min is the default value and the value used to for odd/even
    let query = "
        WITH min_config AS (SELECT (CASE WHEN :min = 0 then even else odd END) as min FROM min_layer)
        INSERT INTO layers(layer) SELECT COALESCE(MAX(MAX(layer) + 2, (SELECT min FROM min_config)) + 2, (SELECT min FROM min_config)) FROM layers WHERE (layer % 2) = :min
        RETURNING *;
    ";
    let arg = if is_even { 0 } else { 1 };
    con.query_row(
        query,
        named_params! {":min": arg},
        |row| row.get(0)
    )
}

fn get_layer_data(con: &Connection, layer: i64) -> rusqlite::Result<(i64, String)> {
    let query = "
        SELECT layers.depth_mined, partitions.serialized
        FROM partitions
        INNER JOIN layers ON partitions.layer = layers.layer
        WHERE partitions.layer = :layer;
    ";
    con.query_row(
        query,
        named_params! {":layer": layer},
        |row| Ok((row.get(0)?, row.get(1)?))
    )
}

fn assign_to_layer(con: &Connection, user: &str, layer: i64) -> rusqlite::Result<()> {
    con.execute("INSERT OR REPLACE INTO assignments VALUES (:username, :layer, UNIXEPOCH())", named_params! {
        ":username": user,
        ":layer": layer
    }).map(|_| ())
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
    con.query_row(
        query,
        named_params! {":username": user},
        |row| row.get(0)
    ).optional()
}

fn set_layer_depth(con: &Connection, layer: i64, depth: i64) -> rusqlite::Result<()> {
    let changed = con.execute("UPDATE layers SET depth_mined = :depth WHERE layer = :layer", named_params! {
        ":depth": depth,
        ":layer": layer
    })?;
    if changed < 1 {
        return Err(rusqlite::Error::StatementChangedRows(0))
    }
    Ok(())
}

fn update_assignment(con: &Connection, user: &str) -> rusqlite::Result<()> {
    let changed = con.execute("UPDATE assignments SET last_update = UNIXEPOCH() WHERE username = :user", named_params! {
        ":user": user,
    })?;
    if changed < 1 {
        return Err(rusqlite::Error::StatementChangedRows(0))
    }
    Ok(())
}

fn update_leaderboard(con: &Connection, user: &str, mined: i64) -> rusqlite::Result<()> {
    let query = "
        INSERT INTO leaderboard (username, blocks_mined)
        VALUES (:username, :blocks_mined)
        ON CONFLICT (username)
        DO UPDATE
        SET blocks_mined = blocks_mined + :blocks_mined
    ";
    con.execute(query, named_params! {
        ":username": user,
        ":blocks_mined": mined
    }).map(|_| ())
}

#[derive(Responder)]
#[response(status = 500, content_type = "text/plain")]
struct SqlError {
    message: String
}
impl SqlError {
    fn new(err: rusqlite::Error) -> Self {
        SqlError { message: format!("Sqlite Error: {}\n", err.to_string()) }
    }
    fn with_msg(msg: &str, err: rusqlite::Error) -> Self {
        SqlError { message: format!("{}\nSqlite Error: {}\n", msg, err.to_string()) }
    }
}
fn err_with_msg(msg: &str) -> impl Fn(rusqlite::Error) -> SqlError + '_ {
    |err| SqlError::with_msg(msg, err)
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct AssignResult {
    depth_mined: i64,
    serialized: String // cringe data lol
}

// restart means this user has just finished a layer
#[get("/assign/<user>/<even_or_odd>/<restart>")]
fn assign(state: &State<Mutex<Connection>>, user: &str, even_or_odd: &str, restart: i32) -> Result<Option<Json<AssignResult>>, SqlError> {
    let is_even = match even_or_odd {
        "even" => true,
        "odd" => false,
        _ => return Ok(None) // 404
    };

    let mut con = state.lock().unwrap();

    let layer = (if restart == 1 {
        assign_to_next_layer(&mut con, user, is_even)
    } else {
        let existing = get_existing_assignment(&con, user);
        match existing {
            Ok(Some(layer)) => Ok(layer),
            Ok(None) => assign_to_next_layer(&mut con, user, is_even),
            Err(err) => Err(err)
        }
    }).map_err(SqlError::new)?;
    let (depth, data) = get_layer_data(&con, layer).map_err(SqlError::new)?;
    Ok(Some(Json(AssignResult{
        depth_mined: depth,
        serialized: data
    })))
}

#[put("/update/<layer>/<depth>")]
fn update_layer(state: &State<Mutex<Connection>>, layer: i64, depth: i64) -> Result<(), SqlError> {
    let con = state.lock().unwrap();
    set_layer_depth(&con, layer, depth).map_err(err_with_msg("set_layer_depth"))
}

// combined leaderboard/update endpoint because otherwise they would both always be called at the same time separately
#[put("/update/<layer>/<depth>/<user>/<blocks>")]
fn update_layer_and_leaderboard(state: &State<Mutex<Connection>>, layer: i64, depth: i64, user: &str, blocks: i64) -> Result<(), SqlError> {
    let mut con = state.lock().unwrap();
    let tx = con.transaction().map_err(SqlError::new)?;
    set_layer_depth(&tx, layer, depth).map_err(err_with_msg("set_layer_depth"))?;
    update_assignment(&tx, user).map_err(err_with_msg("update_assignment"))?;
    update_leaderboard(&tx, user, blocks).map_err(err_with_msg("update_leaderboard"))?;
    tx.commit().map_err(SqlError::new)
}

#[put("/finish/<user>")]
fn finish_layer(state: &State<Mutex<Connection>>, user: &str) -> Result<(), SqlError> {
    let query = "DELETE FROM assignments WHERE username = ?";
    let con = state.lock().unwrap();
    let result = con.execute(query, params![user]);
    result.map(|_| ()).map_err(SqlError::new)
}

#[put("/leaderboard/<user>/<value>")]
fn add_to_leaderboard(state: &State<Mutex<Connection>>, user: &str, value: i64) -> Result<(), SqlError> {
    let con = state.lock().unwrap();
    update_leaderboard(&con, user, value).map_err(SqlError::new)
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct LeaderboardEntry {
    username: String,
    blocks_mined: i64
}

#[get("/leaderboard")]
fn leaderboard(state: &State<Mutex<Connection>>) -> Result<Json<Vec<LeaderboardEntry>>, SqlError> {
    let query = "SELECT username, blocks_mined FROM leaderboard ORDER BY blocks_mined DESC";
    let con = state.lock().unwrap();
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
fn rocket() -> _ {
    let connection = Connection::open("bepitone.db").expect("Failed to open sqlite database (bepitone.db)");

    schema::apply_schema(&connection);

    let rocket = rocket::build()
        .manage(Mutex::new(connection));
    let figment = rocket.figment().clone()
        .merge((Config::PORT, 6969));
        //.merge((Config::ADDRESS, "0.0.0.0"));
    rocket.configure(figment)
        .mount("/", routes![assign, update_layer, update_layer_and_leaderboard, finish_layer, leaderboard, add_to_leaderboard])
}
