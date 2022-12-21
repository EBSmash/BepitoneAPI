mod schema;

#[macro_use]
extern crate rocket;

use rocket::{Config, State};
use rocket::serde::{Serialize, json::Json};
// 1.3.1
use std::sync::Mutex;
use rocket::response::Responder;
use rusqlite::{Connection, named_params, OptionalExtension, params, Transaction};
use indoc::indoc;

fn next_layer(con: &Connection, is_even: bool) -> rusqlite::Result<i64> {
    // min is the default value and the value used to for odd/even
    let query = indoc!{"
        WITH min_config AS (SELECT (CASE WHEN :parity = 0 then even else odd END) as min FROM min_layer)
        INSERT INTO layers(layer) SELECT COALESCE(MAX(MAX(layer) + 2, (SELECT min FROM min_config)) + 2, (SELECT min FROM min_config)) FROM layers WHERE (layer % 2) = :parity
        RETURNING *;
    "};
    let parity = if is_even { 0 } else { 1 };
    con.query_row(
        query,
        named_params! {":parity": parity},
        |row| row.get(0)
    )
}

fn get_layer_data(con: &Connection, layer: i64) -> rusqlite::Result<(Option<i64>, String)> {
    let query = indoc!{"
        SELECT layers.depth_mined, partitions.serialized
        FROM partitions
        INNER JOIN layers ON partitions.layer = layers.layer
        WHERE partitions.layer = :layer;
    "};
    con.query_row(
        query,
        named_params! {":layer": layer},
        |row| Ok((row.get(0)?, row.get(1)?))
    )
}

// creates a new row or replaces that user's existing row
// if a row for a different user exists but has the same layer, overwrite it and make it ours
fn assign_to_layer(tx: &Transaction, user: &str, layer: i64) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM assignments WHERE layer = ?", params![layer]).map(|_| ())?;
    tx.execute("INSERT OR REPLACE INTO assignments VALUES (:username, :layer, UNIXEPOCH())", named_params! {
        ":username": user,
        ":layer": layer,
    }).map(|_| ())
}

fn choose_existing_assignment(con: &Connection, user: &str, is_even: bool) -> rusqlite::Result<Option<(String, i64)>> {
    let query = indoc!{"
        SELECT username,layer
        FROM assignments
        JOIN layers ON ass.layer = layers.layer AND layers.finished = 0 -- only if the layer is unfinished
        WHERE username = :user OR UNIXEPOCH() - last_update > 43200 -- 12 hours
        ORDER BY IIF(username = :user, 0, 1), -- us first
                 IIF(layer % 2 = :parity, 0, 1) -- prefer the same parity
        LIMIT 1
    "};
    con.query_row(
        query,
        named_params! {
            ":user": user,
            ":parity": if is_even { 0 } else { 1 }
        },
        |row| Ok((row.get(0)?, row.get(1)?))
    ).optional()
}

// because we send truncated layer data, the client doesn't know the absolute depth it's mining at so we need to work in relative terms
// (I don't think this is ideal)
fn add_to_layer_depth(con: &Connection, layer: i64, depth: i64) -> rusqlite::Result<()> {
    let changed = con.execute("UPDATE layers SET depth_mined = depth_mined + :depth WHERE layer = :layer", named_params! {
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
    let query = indoc!{"
        INSERT INTO leaderboard (username, blocks_mined)
        VALUES (:username, :blocks_mined)
        ON CONFLICT (username)
        DO UPDATE
        SET blocks_mined = blocks_mined + :blocks_mined
    "};
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
    fn with_msg(msg: &str, err: rusqlite::Error) -> Self {
        SqlError { message: format!("{}\nSqlite Error: {}\n", msg, err.to_string()) }
    }
}
impl From<rusqlite::Error> for SqlError {
    fn from(err: rusqlite::Error) -> Self {
        SqlError { message: format!("Sqlite Error: {}\n", err.to_string()) }
    }
}
trait ToSerializableSqlError<T> {
    fn to_http(self) -> Result<T, SqlError>;
    fn with_msg(self, str: &str) -> Result<T, SqlError>;
}
impl<T> ToSerializableSqlError<T> for Result<T, rusqlite::Error> {
    fn to_http(self) -> Result<T, SqlError> {
        self.map_err(SqlError::from)
    }
    fn with_msg(self, str: &str) -> Result<T, SqlError> {
        self.map_err(|err| SqlError::with_msg(str, err))
    }
}

// restart means this user has just finished a layer
#[get("/assign/<user>/<even_or_odd>")]
fn assign(state: &State<Mutex<Connection>>, user: &str, even_or_odd: &str) -> Result<Option<String>, SqlError> {
    let is_even = match even_or_odd {
        "even" => true,
        "odd" => false,
        _ => return Ok(None) // 404
    };

    let mut con = state.lock().unwrap();
    let tx = con.transaction()?;

    let existing = choose_existing_assignment(&tx, user, is_even)?;
    let (prev_owner, layer) = match &existing {
        Some((owner, layer)) => (Some(owner.as_str()), *layer),
        None => (None, next_layer(&tx, is_even)?)
    };
    assign_to_layer(&tx, user, layer)?;
    let (depth, data) = get_layer_data(&tx, layer).with_msg("No layer data")?;
    tx.commit()?;

    let mut trimmed = String::with_capacity(data.len());
    let mut lines = data.lines();
    let mut first_line = lines.next().unwrap().to_string();

    // if we don't know the state of this layer, or the previous owner made some progress on it, consider it failed
    if depth.is_none() || (depth.unwrap() > 0 && prev_owner == Some(user)) {
        first_line.push_str(".failed");
    }
    trimmed.push_str(first_line.as_str());
    lines.skip(depth.unwrap_or(0) as usize).for_each(|l| {
        trimmed.push_str(l);
        trimmed.push('\n')
    });

    Ok(Some(trimmed))
}

#[post("/update/<layer>/<depth>")]
fn update_layer(state: &State<Mutex<Connection>>, layer: i64, depth: i64) -> Result<(), SqlError> {
    let con = state.lock().unwrap();
    add_to_layer_depth(&con, layer, depth).with_msg("add_to_layer_depth")
}

// combined leaderboard/update endpoint because otherwise they would both always be called at the same time separately
#[post("/update/<layer>/<depth>/<user>/<blocks>")]
fn update_layer_and_leaderboard(state: &State<Mutex<Connection>>, layer: i64, depth: i64, user: &str, blocks: i64) -> Result<(), SqlError> {
    let mut con = state.lock().unwrap();
    let tx = con.transaction()?;
    add_to_layer_depth(&tx, layer, depth).with_msg("add_to_layer_depth")?;
    update_assignment(&tx, user).with_msg("update_assignment")?;
    update_leaderboard(&tx, user, blocks).with_msg("update_leaderboard")?;
    tx.commit().to_http()
}

#[put("/finish/<user>")]
fn finish_layer(state: &State<Mutex<Connection>>, user: &str) -> Result<(), SqlError> {
    let delete = "DELETE FROM assignments WHERE username = ?";
    let set_finished = indoc!{"
        UPDATE layers
        SET finished = 1
        FROM assignments
        WHERE layers.layer = assignments.layer AND assignments.username = ?
    "};
    let mut con = state.lock().unwrap();
    let tx = con.transaction()?;
    for query in [set_finished, delete] {
        tx.execute(query, params![user]).map(|_| ())?
    }
    Ok(())
}

#[post("/leaderboard/<user>/<value>")]
fn add_to_leaderboard(state: &State<Mutex<Connection>>, user: &str, value: i64) -> Result<(), SqlError> {
    let con = state.lock().unwrap();
    update_leaderboard(&con, user, value).to_http()
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
    let mut statement = con.prepare(query)?;

    let rows = statement.query_map([], |row| {
        Ok(LeaderboardEntry {
            username: row.get(0)?,
            blocks_mined: row.get(1)?
        })
    })?;
    let mut entries = Vec::new();
    for entry in rows {
        entries.push(entry?);
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
