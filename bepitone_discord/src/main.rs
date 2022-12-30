use std::{env, thread};
use std::error::Error;
use std::time::Duration;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::model::channel::{Message};
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{StandardFramework, CommandResult};
use hyper::{Body, Request};
use serde::{Deserialize};
use serenity::builder::{EditMessage};
use serenity::model::id::MessageId;
use serenity::model::prelude::ChannelId;
use serenity::model::Timestamp;
use serenity::utils::{Colour};

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

    let token = std::fs::read_to_string("/discord_token").unwrap();
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    let leaderboard_channel = ChannelId(1058258287776247898);
    let leaderboard_msg = MessageId(1058261455859879976);
    let active_users_msg = MessageId(1058285432028278785);
    let http = client.cache_and_http.http.clone();

    loop {
        match query_leaderboard().await {
            Ok(leaderboard) => {
                if let Err(e) = leaderboard_channel.edit_message(&http, leaderboard_msg, |m| {
                    set_leaderboard_data(m, &leaderboard);
                    m
                }).await {
                    println!("Error trying to edit leaderboard {}", e);
                }
            },
            Err(e) => println!("Error query API {}", e)
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
            Err(e) => println!("Error query API {}", e)
        }

        thread::sleep(Duration::from_secs(10));
    }
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