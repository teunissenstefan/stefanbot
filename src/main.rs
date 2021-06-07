//! Requires the "client", "standard_framework", and "voice" features be enabled in your
//! Cargo.toml, like so:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["client", standard_framework", "voice"]
//! ```
// mod song;

use std::env;

extern crate dotenv;

use dotenv::dotenv;
// This trait adds the `register_songbird` and `register_songbird_with` methods
// to the client builder below, making it easy to install this voice client.
// The voice client can be retrieved in any command using `songbird::get(ctx).await`.
use songbird::SerenityInit;

// Import the `Context` to handle commands.
use serenity::client::Context;

use serenity::{
    async_trait,
    client::{Client, EventHandler},
    framework::{
        StandardFramework,
        standard::{
            Args, CommandResult,
            macros::{command, group},
        },
    },
    model::{channel::Message, gateway::Ready},
    Result as SerenityResult,
};
use songbird::input::Input;
use serenity::static_assertions::_core::borrow::BorrowMut;
use std::fs::File;
use std::io::{Write, BufReader, BufRead};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(deafen, join, leave, mute, play, ping, undeafen, unmute, stop, queue, save, load)]
struct General;

#[tokio::main]
async fn main() {
    dotenv().ok();
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c
            .prefix("~"))
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
}

#[command]
#[only_in(guilds)]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel 🥺").await);

            return Ok(());
        }
    };

    let mut handler = handler_lock.lock().await;

    if handler.is_deaf() {
        check_msg(msg.channel_id.say(&ctx.http, "I'm already deafened 😂").await);
    } else {
        if let Err(e) = handler.deafen(true).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "Deafened 😞").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states.get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel 🥺").await);

            return Ok(());
        }
    };

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    let _handler = manager.join(guild_id, connect_to).await;

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel 👋").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel 🥺").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel 🥺").await);

            return Ok(());
        }
    };

    let mut handler = handler_lock.lock().await;

    if handler.is_mute() {
        check_msg(msg.channel_id.say(&ctx.http, "I'm already muted 😂").await);
    } else {
        if let Err(e) = handler.mute(true).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "I am now muted 😞").await);
    }

    Ok(())
}

#[command]
async fn ping(context: &Context, msg: &Message) -> CommandResult {
    check_msg(msg.channel_id.say(&context.http, "Hong Kong long schlong!").await);

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(msg.channel_id.say(&ctx.http, "Must provide a URL to a video or audio").await);

            return Ok(());
        }
    };

    if !url.starts_with("http") {
        check_msg(msg.channel_id.say(&ctx.http, "Must provide a valid URL").await);

        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let mut source = match songbird::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            }
        };
        let msg_title: &str;
        {
            let mut sd = source.borrow_mut();
            // msg_title = sd.metadata.title.as_deref().unwrap_or("song");
            //@TODO get song name when Songbird gets updated to next version
            msg_title = "song";
        }

        check_msg(msg.channel_id.say(&ctx.http, "Playing ".to_owned() + msg_title).await);
        handler.play_source(source);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to play in 🥺").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        handler.stop();
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel 🥺").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.deafen(false).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "Undeafened").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to undeafen in 🥺").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "Unmuted").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to unmute in 🥺").await);
    }

    Ok(())
}

struct Song {
    url: String,
    title: String,
}

#[command]
#[only_in(guilds)]
async fn queue(context: &Context, msg: &Message) -> CommandResult {
    let mut vec: Vec<Song> = load_current_queue().unwrap();
    let mut str: String = "".to_string();
    let mut i: i32 = 1;
    for x in &vec {

        str.push_str(&i.to_string());
        str.push_str(" ");
        str.push_str(&x.title);
        str.push_str("\n");
        i+=1;
    }
    check_msg(msg.channel_id.say(&context.http, "Queue: \n".to_string() + &str).await);

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn save(context: &Context, msg: &Message) -> CommandResult {
    let mut vec: Vec<Song> = load_current_queue().unwrap();
    save_saved_queue(vec);
    check_msg(msg.channel_id.say(&context.http, "Queue saved".to_string()).await);

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn load(context: &Context, msg: &Message) -> CommandResult {
    let mut vec: Vec<Song> = load_saved_queue().unwrap();
    save_current_queue(vec);
    check_msg(msg.channel_id.say(&context.http, "Queue loaded".to_string()).await);

    Ok(())
}

fn save_saved_queue(vec: Vec<Song>) -> std::io::Result<()> {
    let mut write_string: String = "".to_string();
    for x in &vec {
        write_string.push_str(&x.url);
        write_string.push_str("_");
        write_string.push_str(&x.title);
        write_string.push_str("\n");
    }
    let mut file = File::create("saved_queue")?;
    file.write_all(write_string.as_bytes())?;
    Ok(())
}

fn save_current_queue(vec: Vec<Song>) -> std::io::Result<()> {
    let mut write_string: String = "".to_string();
    for x in &vec {
        write_string.push_str(&x.url);
        write_string.push_str("_");
        write_string.push_str(&x.title);
        write_string.push_str("\n");
    }
    let mut file = File::create("current_queue")?;
    file.write_all(write_string.as_bytes())?;
    Ok(())
}

fn load_current_queue() -> std::io::Result<(Vec<Song>)>  {
    let mut vec: Vec<Song> = Vec::new();
    let file = File::open("current_queue")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let (url, title) = split_once(line.unwrap());
        vec.push(Song { url, title });
    }
    Ok(vec)
}

fn load_saved_queue() -> std::io::Result<(Vec<Song>)> {
    let mut vec: Vec<Song> = Vec::new();
    let file = File::open("saved_queue")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let (url, title) = split_once(line.unwrap());
        vec.push(Song { url, title });
    }
    Ok(vec)
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

fn split_once(in_string: String) -> (String, String) {
    let mut splitter = in_string.splitn(2, '_');
    let first = splitter.next().unwrap();
    let second = splitter.next().unwrap();
    (first.parse().unwrap(), second.parse().unwrap())
}