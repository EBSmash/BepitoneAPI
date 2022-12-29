use std::env;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::model::channel::{Message};
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{StandardFramework, CommandResult};
use hyper::{Body, Request};
use serde::{Deserialize};
use serenity::model::Timestamp;
use serenity::utils::Colour;

#[group]
#[commands(leaderboard)]
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
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[derive(Deserialize)]
struct LeaderboardEntry {
    username: String,
    blocks_mined: i64
}

#[command]
async fn leaderboard(ctx: &Context, msg: &Message) -> CommandResult {
    let client = hyper::Client::new();
    let request = Request::get("http://localhost/leaderboard")
        .header("bep-api-key", "48a24e8304a49471404bd036ed7e814bdd59d902d51a47a4bcb090e2fb284f70")
        .body(Body::empty())?;
    let res = client.request(request).await?;
    if res.status() == 200 {
        let buf = hyper::body::to_bytes(res).await?;
        let vec = buf.to_vec();
        let parsed: Vec<LeaderboardEntry> = serde_json::from_slice(&vec)?;
        let channel = msg.channel_id;
        if ![1049092193681420338, 1048030014840516701, 1041084548399771709].contains(&channel.as_u64()) {
            return Ok(())
        }
        channel.send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Bepitone Leaderboard")
                    .color(Colour(0xFFFFFF))
                    .timestamp(Timestamp::now());
                let top_user = parsed.first().map_or("popbob", |entry| entry.username.as_str());
                e.thumbnail(format!("https://minotar.net/bust/{top_user}"));
                let sum: i64 = parsed.iter().map(|e| e.blocks_mined).sum();
                e.description(format!("{sum} blocks mined in total"));
                for LeaderboardEntry { username, blocks_mined} in parsed {
                    if blocks_mined > 0 {
                        e.field(username, blocks_mined, false);
                    }
                }
                e
            })
        }).await?;
    } else {
        msg.reply(ctx,format!("API returned code {}", res.status())).await?;
    }
    Ok(())
}