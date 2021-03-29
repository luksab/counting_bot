#[macro_use]
extern crate lazy_static;

use serenity::framework::standard::{
    macros::{command, group},
    Args, CommandResult, StandardFramework,
};
use serenity::model::{channel::Message, gateway::Ready};
use serenity::model::{channel::ReactionType, id::ChannelId};
use serenity::prelude::*;
use serenity::{
    async_trait,
    model::{
        event::MessageUpdateEvent,
        id::{GuildId, UserId},
    },
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

struct UserKey;

impl TypeMapKey for UserKey {
    type Value = Arc<RwLock<UsersMap>>;
}

type UsersMap = HashMap<UserId, Mutex<UserStats>>;

struct Stats {
    pub correct: u32,
    pub incorrect: u32,
}
struct UserStats {
    pub stats: Stats,
    pub servers: HashMap<GuildId, Stats>,
}

struct ServersKey;

impl TypeMapKey for ServersKey {
    type Value = Arc<RwLock<ServersMap>>;
}

type ServersMap = HashMap<GuildId, Mutex<ServerInfo>>;

struct ServerStats {
    pub correct: u32,
    pub incorrect: u32,
}

struct ServerInfo {
    pub stats: ServerStats,
    pub channel: ChannelId,
    pub lastcount: u32,
    pub last_counter: Option<UserId>,
}

struct Uptime;

impl TypeMapKey for Uptime {
    type Value = std::time::Instant;
}

struct Handler;

async fn do_count(
    ctx: &Context,
    msg: &Message,
    server: &mut ServerInfo,
    user: &mut UserStats,
    correct: bool,
) {
    if correct {
        server.lastcount += 1;
        server.stats.correct += 1;
        user.stats.correct += 1;

        msg.react(&ctx.http, ReactionType::Unicode("✅".to_string()))
            .await
            .expect("send failed!");
        server.last_counter = Some(msg.author.id);
    } else {
        server.lastcount = 0;
        server.stats.incorrect += 1;
        user.stats.incorrect += 1;
        msg.react(&ctx.http, ReactionType::Unicode("❌".to_string()))
            .await
            .expect("send failed!");
        server.last_counter = None;
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Bot ready with username {}", ready.user.name);
    }

    async fn message_update(
        &self,
        _ctx: Context,
        _old_if_available: Option<Message>,
        _new: Option<Message>,
        _event: MessageUpdateEvent,
    ) {
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let ctx_data = ctx.data.read().await;
        let servers_map = ctx_data
            .get::<ServersKey>()
            .expect("Failed to retrieve servers map!")
            .read()
            .await;

        // let users_map = ctx_data
        //     .get::<UserKey>()
        //     .expect("Failed to retrieve users map!")
        //     .read()
        //     .await;

        let guild_id = msg.guild_id.expect("guild id not found");
        let server = servers_map.get(&guild_id);
        match server {
            Some(server) => {
                let mut server = server.lock().await;
                if server.channel == msg.channel_id {
                    if !ctx_data
                        .get::<UserKey>()
                        .expect("Failed to retrieve users map!")
                        .read()
                        .await
                        .contains_key(&msg.author.id)
                    {
                        let mut users_map = ctx_data
                            .get::<UserKey>()
                            .expect("Failed to retrieve users map!")
                            .write()
                            .await;
                        let user = Mutex::new(UserStats {
                            stats: Stats {
                                correct: 0,
                                incorrect: 0,
                            },
                            servers: HashMap::new(),
                        });

                        users_map.insert(msg.author.id, user);
                    }
                    let users_map = ctx_data
                        .get::<UserKey>()
                        .expect("Failed to retrieve users map!")
                        .read()
                        .await;

                    let mut user = users_map
                        .get(&msg.author.id)
                        .expect("inserting new user failed!")
                        .lock()
                        .await;

                    match msg.content.parse::<u32>() {
                        Ok(count) => {
                            if server.lastcount + 1 == count {
                                match server.last_counter {
                                    Some(last_counter) => {
                                        if last_counter != msg.author.id {
                                            do_count(&ctx, &msg, &mut *server, &mut *user, true)
                                                .await;
                                        } else {
                                            do_count(&ctx, &msg, &mut *server, &mut *user, false)
                                                .await;
                                            msg.channel_id
                                                .say(&ctx.http, "You can't count twice in a row!")
                                                .await
                                                .expect("send failed!");
                                        }
                                    }
                                    None => {
                                        do_count(&ctx, &msg, &mut *server, &mut *user, true).await;
                                    }
                                }
                            } else {
                                do_count(&ctx, &msg, &mut *server, &mut *user, false).await;
                                msg.channel_id
                                    .say(&ctx.http, "Incorrect number")
                                    .await
                                    .expect("send failed!");
                            }
                        }
                        Err(_) => {
                            do_count(&ctx, &msg, &mut *server, &mut *user, false).await;
                            msg.channel_id
                                .say(&ctx.http, "That's not a number")
                                .await
                                .expect("send failed!");
                        }
                    }
                }
            }
            None => {
                // msg.channel_id
                //     .say(&ctx.http, "this Server is not configured")
                //     .await
                //     .expect("send failed!");
            }
        }
    }
}

#[group]
#[commands(stats, ping, uptime, here)]
struct General;

#[command]
async fn ping(ctx: &Context, msg: &Message, mut _args: Args) -> CommandResult {
    //msg.channel_id.say(&ctx.http, "Pong!").await?;
    let time = chrono::offset::Utc::now().signed_duration_since(msg.timestamp);

    msg.reply_ping(&ctx.http, format!("Pong! {}ms", time.num_milliseconds()))
        .await?;

    Ok(())
}

#[command]
async fn uptime(ctx: &Context, msg: &Message, mut _args: Args) -> CommandResult {
    //msg.channel_id.say(&ctx.http, "Pong!").await?;
    let uptime = ctx.data.read().await.get::<Uptime>().unwrap().elapsed();
    let seconds = uptime.as_secs();
    let days = seconds / (60 * 60 * 24);
    let hours = seconds % (60 * 60 * 24) / (60 * 60);
    let minutes = seconds % (60 * 60) / 60;
    let seconds = seconds % 60;
    if days == 0 {
        let response = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
        msg.reply_ping(&ctx.http, response).await?;
    } else {
        let response = format!("{:02}days {:02}:{:02}:{:02}", days, hours, minutes, seconds);
        msg.reply_ping(&ctx.http, response).await?;
    }

    Ok(())
}

#[command]
async fn stats(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let users_data = ctx.data.read().await;
    let users_map = users_data
        .get::<UserKey>()
        .expect("Failed to retrieve users map!")
        .read()
        .await;

    let user = users_map.get(&msg.author.id);

    let stat_type = args.single_quoted::<String>();
    match stat_type {
        Ok(user_id) => {
            lazy_static! {
                static ref RE: regex::Regex = regex::Regex::new(r"<@!([0-9]+)>").unwrap();
            }
            let capture = RE.captures(&user_id);

            match capture {
                Some(capture) => {
                    let user = users_map.get(&UserId(
                        capture.get(1).unwrap().as_str().parse::<u64>().unwrap(),
                    ));
                    match user {
                        Some(user) => {
                            let user = user.lock().await;
                            let response =
                                format!("You have {} correct numbers", user.stats.correct);
                            msg.channel_id.say(&ctx.http, &response).await?;
                        }
                        None => {
                            msg.channel_id
                                .say(
                                    &ctx.http,
                                    format!("No stats about {}.", capture.get(0).unwrap().as_str()),
                                )
                                .await?;
                        }
                    }
                }
                None => {
                    msg.channel_id
                        .say(
                            &ctx.http,
                            "You need to mention the user you want stats about.",
                        )
                        .await?;
                }
            }
        }
        Err(_) => match user {
            Some(user) => {
                let user = user.lock().await;
                let response = format!("You have {} correct numbers", user.stats.correct);
                msg.channel_id.say(&ctx.http, &response).await?;
            }
            None => {
                msg.channel_id.say(&ctx.http, "No stats about you.").await?;
            }
        },
    }

    Ok(())
}

#[command]
async fn here(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let member = msg.member(&ctx.http).await?;
    let perm = member.permissions(&ctx).await?;

    let message;

    if perm.administrator() {
        let servers_data = ctx.data.read().await;
        let mut servers_map = servers_data
            .get::<ServersKey>()
            .expect("Failed to retrieve servers map!")
            .write()
            .await;
        let guild_id = msg.guild_id.expect("guild id not found");
        let server = servers_map.get_mut(&guild_id);
        match server {
            Some(server) => {
                let mut server = server.lock().await;
                server.channel = msg.channel_id;
            }
            None => {
                servers_map.insert(
                    guild_id,
                    Mutex::new(ServerInfo {
                        stats: ServerStats {
                            correct: 0,
                            incorrect: 0,
                        },
                        channel: msg.channel_id,
                        lastcount: 0,
                        last_counter: None,
                    }),
                );
            }
        }
        message = msg
            .channel_id
            .say(&ctx.http, "I will consider this the counting channel.");
    } else {
        message = msg
            .channel_id
            .say(&ctx.http, "This command is for admins only!");
    }
    //await later to drop the mutex, RwLock earlier
    message.await?;
    Ok(())
}

//https://discord.com/api/oauth2/authorize?client_id=825121428159332352&permissions=268446784&scope=bot
#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN to be set!");

    let framework = StandardFramework::new()
        .configure(|c| c.case_insensitivity(true).prefix("c$"))
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .type_map_insert::<ServersKey>(Arc::new(RwLock::new(ServersMap::new())))
        .type_map_insert::<UserKey>(Arc::new(RwLock::new(UsersMap::new())))
        .type_map_insert::<Uptime>(std::time::Instant::now())
        .await
        .expect("Failed to build client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
