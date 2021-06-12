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
use std::fs::File;
use std::io::{Write, BufReader, BufRead};
use songbird::tracks::{TrackHandle};
use regex::Regex;
use std::collections::HashMap;

struct Handler;

const COMMAND_PREFIX: &str = "~";

const NOT_IN_VOICE_CHANNEL: &str = "Not in a voice channel 🥺";
const LEFT_VOICE_CHANNEL: &str = "Left voice channel 👋";
const SONGBIRD_INITIALISATION: &str = "Songbird Voice client placed in at initialisation.";
const MUST_PROVIDE_URL: &str = "Must provide a URL to a video or audio";
const MUST_PROVIDE_VALID_URL: &str = "Must provide a valid URL";
const ERROR_SOURCING_FFMPEG: &str = "Error sourcing ffmpeg";
const MUST_PROVIDE_NAME: &str = "Must provide a name";
const MUST_PROVIDE_VALID_NAME: &str = "Must provide a valid name [a-z]";

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(join, leave, play, ping, queue, save, load, pause, resume, skip, clear, help)]
struct General;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c
            .prefix(COMMAND_PREFIX))
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
            check_msg(msg.reply(ctx, NOT_IN_VOICE_CHANNEL).await);

            return Ok(());
        }
    };

    let manager = songbird::get(ctx).await
        .expect(SONGBIRD_INITIALISATION).clone();

    let _handler = manager.join(guild_id, connect_to).await;

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect(SONGBIRD_INITIALISATION).clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, LEFT_VOICE_CHANNEL).await);
    } else {
        check_msg(msg.reply(ctx, NOT_IN_VOICE_CHANNEL).await);
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
            check_msg(msg.channel_id.say(&ctx.http, MUST_PROVIDE_URL).await);

            return Ok(());
        }
    };

    if !url.starts_with("http") {
        check_msg(msg.channel_id.say(&ctx.http, MUST_PROVIDE_VALID_URL).await);

        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let source = match songbird::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, ERROR_SOURCING_FFMPEG).await);

                return Ok(());
            }
        };
        let msg_title: &str;
        {
            // let mut sd = source.borrow_mut();
            // msg_title = sd.metadata.title.as_deref().unwrap_or("song");
            //@TODO get song name when Songbird gets updated to next version
            msg_title = "song";
        }

        check_msg(msg.channel_id.say(&ctx.http, "Added ".to_owned() + msg_title + " to the queue").await);
        handler.enqueue_source(source);
        // handler.play_source(source);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        check_msg(msg.channel_id.say(&ctx.http, "Skipping song").await);
        handler.queue().skip().expect("Could not skip");
    } else {
        check_msg(msg.channel_id.say(&ctx.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    let mut help_map = HashMap::new();
    help_map.insert("join".to_string(), "Make the bot join your voice channel.".to_string());
    help_map.insert("leave".to_string(), "Make the bot leave your voice channel.".to_string());
    help_map.insert("play URL".to_string(), "Add a track to the queue.".to_string());
    help_map.insert("queue".to_string(), "Show all tracks in the queue.".to_string());
    help_map.insert("save NAME".to_string(), "Save the current queue with a name [a-z].".to_string());
    help_map.insert("load NAME".to_string(), "Load the queue with the provided name [a-z].".to_string());
    help_map.insert("pause".to_string(), "Pause playback.".to_string());
    help_map.insert("resume".to_string(), "Resume playback.".to_string());
    help_map.insert("skip".to_string(), "Skip the current track.".to_string());
    help_map.insert("clear".to_string(), "Clear the current queue.".to_string());
    help_map.insert("help".to_string(), "Display all possible commands.".to_string());

    let mut str: String = "Possible commands: ".to_string();
    for (command, description) in &help_map {
        str.push_str("\n");
        str.push_str(COMMAND_PREFIX);
        str.push_str(command);
        str.push_str(": ");
        str.push_str(description);
    }
    check_msg(msg.channel_id.say(&ctx.http, str).await);

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn pause(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().current().unwrap().pause().expect("Could not pause");
        check_msg(msg.channel_id.say(&ctx.http, "Pausing").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().current().unwrap().play().expect("Could not play");
        check_msg(msg.channel_id.say(&ctx.http, "Resuming").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn queue(context: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }

        let mut str: String = "Now playing: ".to_string();
        let mut i: i32 = 0;
        let vec = handler.queue().current_queue();
        let mut total_secs: f64 = 0.0;
        for x in &vec {
            if i != 0 {
                str.push_str(&i.to_string());
                str.push_str(" ");
                str.push_str(x.metadata().title.as_ref().unwrap());
                str.push_str(" 🕑 ");
                str.push_str(&get_time_string(x.metadata().duration.as_ref().unwrap().as_secs() as f64));
                str.push_str("s\n");
            } else {
                str.push_str(x.metadata().title.as_ref().unwrap());
                str.push_str(" 🕑 ");
                str.push_str(&get_time_string(x.metadata().duration.as_ref().unwrap().as_secs() as f64));
                str.push_str("s\n");
                str.push_str("\nQueue: \n");
            }
            total_secs += x.metadata().duration.as_ref().unwrap().as_secs() as f64;
            i += 1;
        }
        str.push_str("\nTotal time: ");
        str.push_str(&get_time_string(total_secs));
        check_msg(msg.channel_id.say(&context.http, &str).await);
    } else {
        check_msg(msg.channel_id.say(&context.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn clear(context: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }

        handler.queue().stop();
        check_msg(msg.channel_id.say(&context.http, "Queue cleared").await);
    } else {
        check_msg(msg.channel_id.say(&context.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn save(context: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let name = match args.single::<String>() {
        Ok(name) => name,
        Err(_) => {
            check_msg(msg.channel_id.say(&context.http, MUST_PROVIDE_NAME).await);

            return Ok(());
        }
    };

    if !check_queue_name(&name) {
        check_msg(msg.channel_id.say(&context.http, MUST_PROVIDE_VALID_NAME).await);

        return Ok(());
    }

    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }
        save_queue(name, handler.queue().current_queue()).expect("Could not save queue");
        check_msg(msg.channel_id.say(&context.http, "Queue saved".to_string()).await);
    } else {
        check_msg(msg.channel_id.say(&context.http, NOT_IN_VOICE_CHANNEL).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn load(context: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let name = match args.single::<String>() {
        Ok(name) => name,
        Err(_) => {
            check_msg(msg.channel_id.say(&context.http, MUST_PROVIDE_NAME).await);

            return Ok(());
        }
    };

    if !check_queue_name(&name) {
        check_msg(msg.channel_id.say(&context.http, MUST_PROVIDE_VALID_NAME).await);

        return Ok(());
    }
    let guild = msg.guild(&context.cache).await.unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(context).await
        .expect(SONGBIRD_INITIALISATION).clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(msg.channel_id.say(&context.http, format!("Failed: {:?}", e)).await);
        }
        check_msg(msg.channel_id.say(&context.http, "Loading queue".to_string()).await);
        let vec = load_queue(name).unwrap_or(Vec::new());
        for x in &vec {
            let source = match songbird::ytdl(&x).await {
                Ok(source) => source,
                Err(why) => {
                    println!("Err starting source: {:?}", why);

                    check_msg(msg.channel_id.say(&context.http, ERROR_SOURCING_FFMPEG).await);

                    return Ok(());
                }
            };
            handler.enqueue_source(source);
        }
        check_msg(msg.channel_id.say(&context.http, "Queue loaded".to_string()).await);
    } else {
        check_msg(msg.channel_id.say(&context.http, NOT_IN_VOICE_CHANNEL).await);
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

fn load_queue(name: String) -> std::io::Result<Vec<String>> {
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