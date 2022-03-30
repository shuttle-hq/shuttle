use serenity::prelude::*;
use serenity::{
    async_trait,
    client::bridge::gateway::{GatewayIntents, ShardId, ShardManager},
    framework::standard::{
        buckets::{LimitedFor, RevertBucket},
        help_commands,
        macros::{check, command, group, help, hook},
        Args,
        CommandGroup,
        CommandOptions,
        CommandResult,
        DispatchError,
        HelpOptions,
        Reason,
        StandardFramework,
    },
    http::Http,
    model::{
        channel::{Channel, Message},
        gateway::Ready,
        id::UserId,
        permissions::Permissions,
    },
    utils::{content_safe, ContentSafeOptions},
};
use tokio::sync::Mutex;

use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Write,
    sync::Arc,
};

use serenity::futures::StreamExt;
use serenity::model::guild::MembersIter;

static TEST_PILOTS_GOAL: u32 = 25;

#[tokio::main]
async fn main() {
   
    let token = option_env!("DISCORD_TOKEN").expect("Discord token not found");

    let http = Http::new_with_token(token);

    let mut owners = HashSet::new();

    let bot_id;

    match http.get_current_application_info().await
    {

        Ok(val) =>
        {

            if let Some(team) = val.team
            {

                owners.insert(team.owner_user_id);

            }
            else {

                owners.insert(val.owner.id);
                
            }

            match http.get_current_user().await
            {

                Ok(val) =>
                {

                    bot_id = val.id;

                }
                Err(err) =>
                {

                    panic!("Bot id error: {:?}", err);

                }

            }

        }
        Err(err) =>
        {

            panic!("Application info error: {:?}", err);

        }

    }

    let mut framework = StandardFramework::new().configure(|config| {

        config.with_whitespace(true).on_mention(Some(bot_id)).owners(owners)
        
        //.prefix(prefix)

    });
    
    framework = framework.group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
    .event_handler(Handler)
    .framework(framework)
    .intents(GatewayIntents::all())
    .await
    .expect("Error creating client");

    if let Err(err) = client.start().await
    {

        println!("Client failed to start: {}", err);

    }

}

struct Handler;

#[async_trait]
impl EventHandler for Handler
{

    async fn ready(&self, _: Context, ready: Ready)
    {

        println!("{} is connected!", ready.user.name);

    }

}

#[group]
#[commands(show_test_pilot_goal)]
struct General;

#[command]
#[description = "Outputs the number test-pilots and how far the shuttle discord server is from reaching 25"]
async fn show_test_pilot_goal(ctx: &Context, msg: &Message, mut _args: Args) -> CommandResult
{

    let guild_id_option = msg.guild_id;

    let guild_id;
    
    match guild_id_option {

        Some(val) => {

            guild_id = val;

        }
        None => {

            msg.channel_id.say(&ctx.http, "Error command not executed on server").await?;

            return Ok(());

        }
        
    }

    let mut test_pilots_count: u32 = 0;

    //get the members

    let mut members = guild_id.members_iter(&ctx).boxed();

    while let Some(member_result) = members.next().await
    {

        match member_result {

            Ok(val) => {

                if let Some(roles) = val.roles(ctx.cache.clone()).await
                {

                    for role in roles {
                    
                        if role.name == "Test Pilot"
                        {

                            test_pilots_count += 1;

                            break;

                        }

                    }

                }

            }
            Err(err) => {

                msg.channel_id.say(&ctx.http, err.to_string()).await?;

                return Ok(());

            }

        }

    }

    if test_pilots_count < TEST_PILOTS_GOAL
    {

        msg.channel_id.say(&ctx.http, format!("'{}' to go!", TEST_PILOTS_GOAL - test_pilots_count)).await?;

    }
    else if test_pilots_count == TEST_PILOTS_GOAL
    {

        msg.channel_id.say(&ctx.http, format!("Goal of '{}' Test Pilots reached!", TEST_PILOTS_GOAL)).await?;

    }
    else {

        let amount = test_pilots_count - TEST_PILOTS_GOAL;

        msg.channel_id.say(&ctx.http, format!("Goal of '{}' Test Pilots exceeded by {n}!", TEST_PILOTS_GOAL, n = amount)).await?;
        
    }

    Ok(())

}
