mod schema;

#[macro_use]
extern crate rocket;

use rocket::{Config, Request, State};
use rocket::serde::{Serialize, json::Json};
// 1.3.1
use std::sync::Mutex;
use rocket::response::Responder;
use rusqlite::{Connection, named_params, OptionalExtension, params, Transaction};
use indoc::indoc;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};

fn next_layer(con: &Connection, is_even: bool) -> rusqlite::Result<i64> {
    // min is the default value and the value used to for odd/even
    let query = indoc!{"
        WITH min_config AS (SELECT (CASE WHEN :parity = 0 then even else odd END) as min FROM min_layer)
        INSERT INTO layers(layer) SELECT COALESCE(MAX(MAX(layer) + 2, (SELECT min FROM min_config)), (SELECT min FROM min_config)) FROM layers WHERE (layer % 2) = :parity
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
        SELECT username,assignments.layer
        FROM assignments
        JOIN layers ON assignments.layer = layers.layer AND layers.finished = 0 -- only if the layer is unfinished
        WHERE (username = :user OR (UNIXEPOCH() - last_update > 86400 AND assignments.layer % 2 = :parity)) -- 24 hours
        ORDER BY IIF(username = :user, 0, 1) -- us first
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

fn get_failed_layer(con: &Connection, is_even: bool) -> rusqlite::Result<Option<i64>> {
    let query = indoc!{"
        WITH min_config AS (SELECT (CASE WHEN :parity = 0 then even else odd END) as min FROM min_layer)
        SELECT layer FROM layers
        WHERE depth_mined IS NULL
              AND (layer % 2 = :parity)
              AND layer >= (SELECT min FROM min_config)
              AND finished = 0
              AND layer NOT IN (SELECT layer from assignments)
        LIMIT 1
    "};
    let parity = if is_even { 0 } else { 1 };
    con.query_row(query, named_params! {":parity": parity}, |row| row.get(0)).optional()
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


struct ApiKey();

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ApiKey {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        #[cfg(debug_assertions)]
        return Outcome::Success(ApiKey());

        // if this specific string is not in the request headers, pretend the api doesn't exist
        match req.headers().get_one("bep-api-key") {
            None => Outcome::Failure((Status::NotFound, "missing api key")),
            Some("48a24e8304a49471404bd036ed7e814bdd59d902d51a47a4bcb090e2fb284f70") => Outcome::Success(ApiKey()),
            Some(_) => Outcome::Failure((Status::NotFound, "wrong api key")),
        }
    }
}

#[put("/insert_layer/<layer>/<finished>?<depth>")]
fn insert_layer(_key: ApiKey, state: &State<Mutex<Connection>>, layer: i64, depth: Option<i64>, finished: bool) -> Result<(), SqlError> {
    let db = state.lock().unwrap();
    db.execute("INSERT OR REPLACE INTO layers VALUES (?, ?, ?)", params![layer, depth, finished]).map(|_| ()).to_http()
}

#[put("/assign/<user>/<even_or_odd>")]
fn assign(_key: ApiKey, state: &State<Mutex<Connection>>, user: &str, even_or_odd: &str) -> Result<Option<String>, SqlError> {
    let is_even = match even_or_odd {
        "even" => true,
        "odd" => false,
        _ => return Ok(None) // 404
    };

    let mut con = state.lock().unwrap();
    let tx = con.transaction()?;

    let existing = choose_existing_assignment(&tx, user, is_even)?;

    let layer = if matches!(&existing, Some((owner, _)) if owner == user) {
        existing.unwrap().1
    } else if let Some(failed) = get_failed_layer(&tx, is_even)? {
        failed
    } else if let Some((_, layer)) = &existing {
        *layer
    } else {
        next_layer(&tx, is_even)?
    };

    assign_to_layer(&tx, user, layer)?;
    let (depth, data) = get_layer_data(&tx, layer).with_msg("No layer data")?;
    tx.commit()?;

    let mut lines = data.lines();
    let first_line = lines.next().unwrap();

    let mut trimmed = String::with_capacity(data.len());
    trimmed.push_str(first_line); trimmed.push('\n');

    // if we don't know the state of this layer, consider it failed
    trimmed.push_str(format!("failed={}\n", depth.is_none()).as_str());
    trimmed.push_str(format!("depth={}\n", depth.unwrap_or(0)).as_str());

    lines.skip(depth.unwrap_or(0) as usize).for_each(|l| {
        trimmed.push_str(l);
        trimmed.push('\n')
    });

    Ok(Some(trimmed))
}

#[post("/update/<layer>/<depth>")]
fn update_layer(_key: ApiKey, state: &State<Mutex<Connection>>, layer: i64, depth: i64) -> Result<(), SqlError> {
    let con = state.lock().unwrap();
    set_layer_depth(&con, layer, depth).with_msg("set_layer_depth")
}

// combined leaderboard/update endpoint because otherwise they would both always be called at the same time separately
#[post("/update/<layer>/<depth>/<user>/<blocks>")]
fn update_layer_and_leaderboard(_key: ApiKey, state: &State<Mutex<Connection>>, layer: i64, depth: i64, user: &str, blocks: i64) -> Result<(), SqlError> {
    let mut con = state.lock().unwrap();
    let tx = con.transaction()?;
    set_layer_depth(&tx, layer, depth).with_msg("set_layer_depth")?;
    update_assignment(&tx, user).with_msg("update_assignment")?;
    update_leaderboard(&tx, user, blocks).with_msg("update_leaderboard")?;
    tx.commit().to_http()
}

#[put("/finish/<layer>")]
fn finish_layer(_key: ApiKey, state: &State<Mutex<Connection>>, layer: i64) -> Result<(), SqlError> {
    let delete = "DELETE FROM assignments WHERE layer = ?";
    let set_finished = "UPDATE layers SET finished = 1 WHERE layer = ?";
    let mut con = state.lock().unwrap();
    let tx = con.transaction()?;
    for query in [set_finished, delete] {
        tx.execute(query, params![layer]).map(|_| ())?
    }
    tx.commit()?;
    Ok(())
}

#[post("/leaderboard/<user>/<value>")]
fn add_to_leaderboard(_key: ApiKey, state: &State<Mutex<Connection>>, user: &str, value: i64) -> Result<(), SqlError> {
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
fn leaderboard(_key: ApiKey, state: &State<Mutex<Connection>>) -> Result<Json<Vec<LeaderboardEntry>>, SqlError> {
    let query = indoc!{"
        SELECT COALESCE((SELECT name FROM leaderboard_aliases WHERE account = username), username) AS name, SUM(blocks_mined) FROM leaderboard
        GROUP BY name
        ORDER BY SUM(blocks_mined) DESC
    "};
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

#[get("/active_users")]
fn active_users(_key: ApiKey, state: &State<Mutex<Connection>>) -> Result<String, SqlError> {
    let query = "SELECT username FROM assignments WHERE UNIXEPOCH() - last_update < 180 ORDER BY layer DESC";
    let con = state.lock().unwrap();
    let mut stmnt = con.prepare(query)?;
    let rows = stmnt.query_map([], |row| Ok(row.get(0)?))?;
    let mut out = String::new();
    for row in rows {
        let line: Box<str> = row?;
        out.push_str(line.as_ref());
        out.push('\n');
    }
    Ok(out)
}

#[launch]
fn rocket() -> _ {
    let connection = Connection::open("bepitone.db").expect("Failed to open sqlite database (bepitone.db)");

    schema::apply_schema(&connection);

    let rocket = rocket::build()
        .manage(Mutex::new(connection));
    let mut figment = rocket.figment().clone();
    #[cfg(debug_assertions)]
    let debug = true;
    #[cfg(not(debug_assertions))]
    let debug = false;
    if debug {
        figment = figment
            .merge((Config::PORT, 6969))
            .merge((Config::ADDRESS, "127.0.0.1"));
    } else {
        figment = figment
            .merge((Config::PORT, 80))
            .merge((Config::ADDRESS, "0.0.0.0"));
    }

    rocket.configure(figment)
        .mount("/", routes![assign, update_layer, update_layer_and_leaderboard, finish_layer, leaderboard, add_to_leaderboard, active_users, insert_layer])
}
