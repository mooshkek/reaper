use serde_json::Value;
use serenity::{builder::CreateApplicationCommand, prelude::Context, model::prelude::{interaction::application_command::ApplicationCommandInteraction, command::CommandOptionType}};
use tracing::{error, warn};

use crate::{Handler, commands::{structs::CommandError, utils::messages::send_message}, mongo::structs::{Permissions, ActionType}};

pub async fn run(handler: &Handler, ctx: &Context, cmd: &ApplicationCommandInteraction) -> Result<(), CommandError> {
    let mut user_id: i64 = cmd.user.id.0 as i64;
    let mut expired = false;

    for option in cmd.data.options.iter() {
        match option.kind {
            CommandOptionType::User => {
                match Value::to_string(&option.value.clone().unwrap()).replace("\"", "").parse::<i64>() {
                    Ok(id) => {
                        user_id = id;
                    },
                    Err(err) => {
                        error!("Failed to parse user ID. This is because: {}", err);
                        return Err(CommandError {
                            message: "Failed to parse user ID".to_string(),
                            command_error: None
                        });
                    }
                }
            },
            CommandOptionType::Boolean => {
                expired = option.value.as_ref().unwrap().as_bool().unwrap();
            },
            _ => warn!("Option type {:?} not handled", option.kind)
        }
    }

    let permission;
    if user_id == cmd.user.id.0 as i64 {
        if expired {
            permission = Permissions::ModerationSearchSelfExpired;
        }
        else {
            permission = Permissions::ModerationSearchSelf;
        }
    }
    else {
        if expired {
            permission = Permissions::ModerationSearchOthersExpired;
        }
        else {
            permission = Permissions::ModerationSearchOthers;
        }
    }

    match handler.has_permission(&ctx, &cmd.member.as_ref().unwrap(), permission).await {
        Ok(has_permission) => {
            if !has_permission {
                return handler.missing_permissions(&ctx, &cmd, permission).await
            }
        },
        Err(err) => {
            error!("Failed to check if user has permission to use moderation search command. Failed with error: {}", err);
            return Err(CommandError {
                message: "Failed to check if user has permission to use moderation search command".to_string(),
                command_error: None
            });
        }
    }

    match handler.mongo.get_actions_for_user(user_id, cmd.guild_id.unwrap().0 as i64).await {
        Ok(actions) => {
            let mut message_content = format!("<@{}>'s history:\n", user_id);
            let mut active_actions = 0;
            for action in actions.iter() {
                if action.active || expired {
                    active_actions += 1;
                    message_content.push_str(&format!("\n**{}**", match action.action_type {
                        ActionType::Strike => "Strike",
                        ActionType::Mute => "Mute",
                        ActionType::Kick => "Kick",
                        ActionType::Ban => "Ban",
                        _ => "Unknown"
                    }));
                    if !action.active {
                        message_content.push_str(" *(Expired)*");
                    }
                    message_content.push_str(&format!("\n{}\n\t*Issued by:* <@{}>", action.reason, action.moderator_id));
                    if let Some(expiry) = action.expiry {
                        message_content.push_str(&format!("\n\t*Expires:* <t:{}:F>", expiry));
                    }
                    message_content.push_str(&format!("\n\t*UUID*: {}\n", action.uuid));
                }
            }
            if active_actions == 0 {
                if expired {
                    message_content.push_str("No history");
                }
                else {
                    message_content.push_str("No active history");
                }
            }
            return send_message(&ctx, &cmd, message_content, Some(user_id == cmd.user.id.0 as i64)).await;
        },
        Err(err) => {
            error!("Failed to get actions for user. Failed with error: {}", err);
            return Err(CommandError {
                message: "Failed to get actions for user".to_string(),
                command_error: None
            });
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("search")
        .dm_permission(false)
        .description("Searches a user for their moderation history")
        .create_option(|option| {
            option
                .name("user")
                .description("The user to search")
                .kind(CommandOptionType::User)
                .required(false)
        })
        .create_option(|option| {
            option
                .name("expired")
                .description("View expired actions?")
                .kind(CommandOptionType::Boolean)
                .required(false)
        })
}