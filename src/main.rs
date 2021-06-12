use std::env;

extern crate dotenv;

use dotenv::dotenv;
use songbird::SerenityInit;
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
use songbird::tracks::{TrackQueue, TrackHandle};
use regex::Regex;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(join, leave, play, ping, queue, save, load)]
struct General;

#[tokio::main]
async fn main() {
    dotenv().ok();
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

        check_msg(msg.channel_id.say(&ctx.http, "Added ".to_owned() + msg_title + " to the queue").await);
        handler.enqueue_source(source);
        // handler.play_source(source);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel 🥺").await);
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
    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }

        let mut str: String = "Queue: \n\n".to_string();
        let mut i: i32 = 1;
        let mut vec = handler.queue().current_queue();
        let mut total_secs: f64 = 0.0;
        for x in &vec {
            str.push_str(&i.to_string());
            str.push_str(" ");
            str.push_str(x.metadata().title.as_ref().unwrap());
            str.push_str(" 🕑 ");
            total_secs += x.metadata().duration.as_ref().unwrap().as_secs() as f64;
            str.push_str(&get_time_string(x.metadata().duration.as_ref().unwrap().as_secs() as f64));
            str.push_str("s\n");
            i += 1;
        }
        str.push_str("\nTotal time: ");
        str.push_str(&get_time_string(total_secs));
        check_msg(msg.channel_id.say(&context.http, &str).await);
    } else {
        check_msg(msg.channel_id.say(&context.http, "Not in a voice channel 🥺").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn save(context: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let name = match args.single::<String>() {
        Ok(name) => name,
        Err(_) => {
            check_msg(msg.channel_id.say(&context.http, "Must provide a name").await);

            return Ok(());
        }
    };

    if !check_queue_name(&name) {
        check_msg(msg.channel_id.say(&context.http, "Must provide a valid name [a-z]").await);

        return Ok(());
    }

    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }
        save_queue(name, handler.queue().current_queue());
        check_msg(msg.channel_id.say(&context.http, "Queue saved".to_string()).await);
    } else {
        check_msg(msg.channel_id.say(&context.http, "Not in a voice channel 🥺").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn load(context: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let name = match args.single::<String>() {
        Ok(name) => name,
        Err(_) => {
            check_msg(msg.channel_id.say(&context.http, "Must provide a name").await);

            return Ok(());
        }
    };

    if !check_queue_name(&name) {
        check_msg(msg.channel_id.say(&context.http, "Must provide a valid name [a-z]").await);

        return Ok(());
    }
    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }
        let vec = load_queue(name).unwrap_or(Vec::new());
        for x in &vec {
            let mut source = match songbird::ytdl(&x).await {
                Ok(source) => source,
                Err(why) => {
                    println!("Err starting source: {:?}", why);

                    check_msg(msg.channel_id.say(&context.http, "Error sourcing ffmpeg").await);

                    return Ok(());
                }
            };
            handler.enqueue_source(source);
        }
        check_msg(msg.channel_id.say(&context.http, "Queue loaded".to_string()).await);
    } else {
        check_msg(msg.channel_id.say(&context.http, "Not in a voice channel 🥺").await);
    }

    Ok(())
}

fn check_queue_name(name: &String) -> bool {
    let re = Regex::new(r"^[a-z]*$").unwrap();
    re.is_match(name.as_ref())
}

fn save_queue(name: String, vec: Vec<TrackHandle>) -> std::io::Result<()> {
    let mut write_string: String = "".to_string();
    for x in &vec {
        write_string.push_str(&x.metadata().source_url.as_ref().unwrap());
        write_string.push_str("\n");
    }
    let mut file = File::create(name + ".list")?;
    file.write_all(write_string.as_bytes())?;
    Ok(())
}

fn load_queue(name: String) -> std::io::Result<(Vec<String>)> {
    let mut vec: Vec<String> = Vec::new();
    let file = File::open(name + ".list")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        vec.push(line.unwrap());
    }
    Ok(vec)
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

fn get_time_string(mut secs: f64) -> String {
    let mut str: String = "".to_string();
    let hours = (secs / 3600.0).floor();
    secs = secs - (hours * 3600.0);
    let minutes = (secs / 60.0).floor();
    secs = secs - (minutes * 60.0);
    if hours > 0.0 {
        str.push_str(hours.to_string().as_str());
        str.push_str(":");
    }
    str.push_str(minutes.to_string().as_str());
    str.push_str(":");
    str.push_str(secs.to_string().as_str());
    return str;
}