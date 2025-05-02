use anyhow::{Error, anyhow};
use async_trait::async_trait;

use xtra::{Context, Handler, Message};

use crate::bot::{ApiCommand, MasterBot};
use crate::command::Command;
use crate::web_server::BotData;

pub struct BotNameListRequest;

impl Message for BotNameListRequest {
    type Result = Vec<String>;
}

#[async_trait]
impl Handler<BotNameListRequest> for MasterBot {
    async fn handle(&mut self, _: BotNameListRequest, _: &mut Context<Self>) -> Vec<String> {
        self.bot_names()
    }
}

pub struct BotDataListRequest;

impl Message for BotDataListRequest {
    type Result = Vec<BotData>;
}

#[async_trait]
impl Handler<BotDataListRequest> for MasterBot {
    async fn handle(&mut self, _: BotDataListRequest, _: &mut Context<Self>) -> Vec<BotData> {
        self.bot_datas().await
    }
}

pub struct BotDataRequest {
    pub token: String,
}

impl Message for BotDataRequest {
    type Result = Option<BotData>;
}

#[async_trait]
impl Handler<BotDataRequest> for MasterBot {
    async fn handle(&mut self, r: BotDataRequest, _: &mut Context<Self>) -> Option<BotData> {
        self.bot_data(&r.token).await
    }
}

pub struct LoginRequest(pub String);

impl Message for LoginRequest {
    type Result = Result<String, anyhow::Error>;
}

#[async_trait]
impl Handler<LoginRequest> for MasterBot {
    async fn handle(
        &mut self,
        r: LoginRequest,
        _: &mut Context<Self>,
    ) -> Result<String, anyhow::Error> {
        let token = r.0;

        self.uid_by_web_token(&token).await
    }
}

pub struct CommandRequest {
    pub token: String,
    pub command: Command,
}

impl Message for CommandRequest {
    type Result = Result<(), Error>;
}

#[async_trait]
impl Handler<CommandRequest> for MasterBot {
    async fn handle(
        &mut self,
        r: CommandRequest,
        _: &mut Context<Self>,
    ) -> <CommandRequest as Message>::Result {
        if let Some(client) = self.client_by_user_token(&r.token).await
            && let Some(bot) = self.bot_by_channel(client.channel).await
        {
            bot.send(ApiCommand {
                command: r.command,
                client,
            })
            .await
            .unwrap()
            .unwrap();
            Ok(())
        } else {
            Err(anyhow!("no bot in your channel"))
        }
    }
}
