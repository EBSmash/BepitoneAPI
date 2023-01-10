use std::env;
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::framework::standard::macros::{group};
use serenity::framework::standard::{StandardFramework};
use serenity::builder::{EditMessage};
use serenity::model::id::MessageId;
use serenity::model::prelude::ChannelId;
use serenity::model::Timestamp;
use serenity::utils::{Colour};
use serenity::model::channel::AttachmentType;
use hyper::{Body, Request};
use rusqlite::Connection;
use serde::{Deserialize};
use tokio::{spawn, task, task::spawn_local, time::interval};

#[group]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix(".")) // set the bot's prefix to "."
        .group(&GENERAL_GROUP);

    let token = env::var("DISCORD_TOKEN").unwrap_or_else(|_| std::fs::read_to_string("/discord_token").unwrap());
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    let leaderboard_channel = ChannelId(1058258287776247898);
    let leaderboard_msg = MessageId(1058261455859879976);
    let active_users_msg = MessageId(1058285432028278785);
    let progress_image_msg = MessageId(1060864851792105482);
    let http = client.cache_and_http.http.clone();

    let render_task = {
        let layer_data = get_partition_data().unwrap();
        let http = http.clone();
        async move {
            let mut interval = interval(Duration::from_secs(60 * 5));
            loop {
                interval.tick().await;
                match read_progress_from_db() {
                    Ok(progress) => {
                        let pixels = render(&layer_data, &progress);
                        let png = create_png(&pixels);
                        if let Err(e) = leaderboard_channel.edit_message(&http, progress_image_msg, |m| {
                            m.content("");
                            m.attachment(AttachmentType::from((png.as_slice(), "progress.png")));
                            m
                        }).await {
                            println!("Error trying to edit progress {}", e);
                        }
                    },
                    Err(e) => println!("Error getting progress from db ({})", e)
                }
            }
        }
    };

    let leaderboard_task = {
        let http = http.clone();
        async move {
            let mut interval = interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                match query_leaderboard().await {
                    Ok(leaderboard) => {
                        if let Err(e) = leaderboard_channel.edit_message(&http, leaderboard_msg, |m| {
                            set_leaderboard_data(m, &leaderboard);
                            m
                        }).await {
                            println!("Error trying to edit leaderboard {}", e);
                        }
                    },
                    Err(e) => println!("Error querying API {}", e)
                };

                match query_active().await {
                    Ok(active) => {
                        if let Err(e) = leaderboard_channel.edit_message(&http, active_users_msg, |m| {
                            set_active_users_embed(m, active.as_str());
                            m
                        }).await {
                            println!("Error trying to edit active_users {}", e)
                        }
                    },
                    Err(e) => println!("Error querying API {}", e)
                }
            };
        }
    };
    spawn(render_task);
    let local = task::LocalSet::new();
    // some cringe shit about dyn Error not being Send
    local.run_until(async move {
        spawn_local(leaderboard_task).await.unwrap();
    }).await;
}

#[derive(Deserialize)]
struct LeaderboardEntry {
    username: String,
    blocks_mined: i64
}

async fn do_query(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let client = hyper::Client::new();
    let request = Request::get(format!("http://localhost{path}"))
        .header("bep-api-key", "48a24e8304a49471404bd036ed7e814bdd59d902d51a47a4bcb090e2fb284f70")
        .body(Body::empty())?;
    let res = client.request(request).await?;
    if res.status() == 200 {
        let buf = hyper::body::to_bytes(res).await?;
        let vec = buf.to_vec();
        Ok(vec)
    } else {
        Err(format!("{path} returned error {}", res.status()))?
    }
}

async fn query_leaderboard() -> Result<Vec<LeaderboardEntry>, Box<dyn Error>> {
    let body = do_query("/leaderboard").await?;
    let out = serde_json::from_slice(&body)?;
    Ok(out)
}

async fn query_active() -> Result<String, Box<dyn Error>> {
    let body = do_query("/active_users").await?;
    let out = String::from_utf8(body)?;
    Ok(out)
}

fn set_active_users_embed(m: &mut EditMessage, data: &str) {
    m.embed(|e| {
        e.title("Active Miners")
            .description(data)
            .color(Colour(0xFFFFFF))
            .timestamp(Timestamp::now());
        e
    });
}

fn set_leaderboard_data(m: &mut EditMessage, data: &Vec<LeaderboardEntry>) {
    m.embed(|e| {
        e.title("Bepitone Leaderboard")
            .color(Colour(0xFFFFFF))
            .timestamp(Timestamp::now());
        let top_user = data.first().map_or("popbob", |entry| entry.username.as_str());
        e.thumbnail(format!("https://minotar.net/bust/{top_user}"));
        let sum: i64 = data.iter().map(|e| e.blocks_mined).sum();
        e.description(format!("{sum} blocks mined in total"));
        for LeaderboardEntry { username, blocks_mined} in data {
            if *blocks_mined > 0 {
                e.field(username, blocks_mined, false);
            }
        }
        e
    });
}

fn create_png(((width, height), pixels): &((i32, i32), Vec<[u8; 4]>)) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut encoder = png::Encoder::new(&mut buf, *width as u32, *height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let as_bytes = unsafe { std::slice::from_raw_parts(std::mem::transmute(pixels.as_ptr()), pixels.len() * 4) };
    writer.write_image_data(as_bytes).unwrap();
    drop(writer);
    buf
}

fn render((layers, max_z): &(Vec<(i32, Vec<(u8, i32)>)>, i32), progress: &HashMap<i64, (bool, i64)>) -> ((i32, i32), Vec<[u8; 4]>) {
    let min_layer = layers.first().unwrap().0;
    let width = (layers.last().unwrap().0 - min_layer) * 5;
    let height = *max_z + 1;

    let mut pixels = vec![[0xFF, 0xFF, 0xFF, 0xFF]; (width * height) as usize];
    for (layer, rows) in layers {
        let limit = match progress.get(&(*layer as i64)) {
            Some((false, depth)) => *depth as usize,
            Some((true, _)) => rows.len(),
            None => 0usize
        };

        for (bits, z) in rows.iter().take(limit) {
            for i in 0..5 {
                if ((bits >> i) & 1) != 0 {
                    let x = ((layer - min_layer) * 5) + i;
                    pixels[((z * width) + x) as usize] = [0, 0, 0, 0xFF];
                }
            }
        }
    }
    ((width, height), pixels)
}

fn get_partition_data() -> rusqlite::Result<(Vec<(i32, Vec<(u8, i32)>)>, i32)> {
    let mut data = read_partitions_from_db()?;
    let max_z = normalize_z(&mut data);
    Ok((data, max_z))
}

fn normalize_z(data: &mut Vec<(i32, Vec<(u8, i32)>)>) -> i32 {
    // this language sucks
    macro_rules! iter { () => { data.iter_mut().flat_map(|(_, layer)| layer.iter_mut().map(|(_, z)| z))}; }
    let min = *iter!().min().unwrap();
    iter!().for_each(|z| *z -= min);
    let max = *iter!().max().unwrap();
    max
}

fn parse_layer(layer: i32, data: &str) -> Vec<(u8, i32)> {
    data.lines().skip(1)
        .map(|line| {
            line.split('#')
                .map(|pair| pair.split_once(' ').unwrap())
                .map(|(a, b)| (a.parse::<i32>().unwrap(), b.parse::<i32>().unwrap()))
                .fold((0u8, 0), |(bits, _), (x, z)| {
                    let x_offset = x - (layer * 5);
                    (bits | (1 << x_offset), z)
                })
        })
        .collect()
}

fn read_partitions_from_db() -> rusqlite::Result<Vec<(i32, Vec<(u8, i32)>)>> {
    let connection = Connection::open("bepitone.db").expect("Failed to open sqlite database (bepitone.db)");
    let query = "SELECT layer,serialized FROM partitions WHERE serialized LIKE '% %' ORDER BY layer";
    let mut statement = connection.prepare(query)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    let mut data = Vec::new();
    for row in rows {
        let row = row?;
        let layer = row.0;
        let serialized: Box<str> = row.1;
        data.push((layer, parse_layer(layer, &serialized)));
    }
    Ok(data)
}

fn read_progress_from_db() -> rusqlite::Result<HashMap<i64, (bool, i64)>> {
    let connection = Connection::open("bepitone.db")?;
    let query = "SELECT layer,depth_mined,finished FROM layers";
    let mut statement = connection.prepare(query)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (layer, depth, finished) = row?;
        let depth: Option<i64> = depth;
        map.insert(layer, (finished, depth.unwrap_or(0)));
    }
    Ok(map)
}