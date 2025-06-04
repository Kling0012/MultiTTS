use serenity::{
    async_trait,
    model::{gateway::Ready, id::GuildId, interaction::{Interaction, InteractionResponseType}},
    prelude::*,
};
use songbird::{SerenityInit, input::ffmpeg};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use dotenv::dotenv;
use std::env;
use urlencoding::encode;

fn build_tts_url(text: &str) -> String {
    format!(
        "https://translate.google.com/translate_tts?ie=UTF-8&q={}&tl=ja&client=tw-ob",
        encode(text)
    )
}

struct BotState;
impl TypeMapKey for BotState {
    type Value = Arc<Mutex<HashMap<GuildId, serenity::model::id::ChannelId>>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("✅ ログイン成功: {}", ready.user.name);
        // ギルドIDを指定（複数ギルドやグローバル化は適宜変更）
        let guild_id = GuildId(
            env::var("DISCORD_GUILD_ID")
                .expect("環境変数 DISCORD_GUILD_ID が設定されていません")
                .parse()
                .expect("DISCORD_GUILD_ID の値が無効です。数値である必要があります"),
        );
        // スラッシュコマンドを登録
        guild_id
            .set_guild_application_commands(&ctx.http, |commands| {
                commands
                    .create_application_command(|cmd| {
                        cmd.name("join").description("VCに参加します")
                    })
                    .create_application_command(|cmd| {
                        cmd.name("leave").description("VCから退出します")
                    })
                    .create_application_command(|cmd| {
                        cmd
                            .name("say")
                            .description("指定したテキストを読み上げます")
                            .create_option(|opt| {
                                opt.kind(serenity::model::application::command::CommandOptionType::String)
                                    .name("text")
                                    .description("読み上げる内容")
                                    .required(true)
                            })
                    })
            })
            .await
            .expect("コマンド登録失敗");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(cmd) = interaction {
            let content = match cmd.data.name.as_str() {
                "join" => {
                    let guild_id = cmd.guild_id.unwrap();
                    if let Some(member) = cmd.member.clone() {
                        if let Some(vc) = member.voice.as_ref().and_then(|v| v.channel_id) {
                            let manager = songbird::get(&ctx).await.unwrap().clone();
                            let _ = manager.join(guild_id, vc).await;
                            // 状態に保存
                            let data = ctx.data.read().await;
                            let map = data.get::<BotState>().unwrap().clone();
                            map.lock().await.insert(guild_id, vc);
                            "✅ ボイスチャネルに参加しました。".to_string()
                        } else {
                            "⚠️ まずVCに参加してください。".to_string()
                        }
                    } else { "⚠️ メンバー情報が取得できません。".to_string() }
                }
                "leave" => {
                    let guild_id = cmd.guild_id.unwrap();
                    let data = ctx.data.read().await;
                    let map = data.get::<BotState>().unwrap().clone();
                    if map.lock().await.remove(&guild_id).is_some() {
                        let manager = songbird::get(&ctx).await.unwrap().clone();
                        let _ = manager.remove(guild_id).await;
                        "👋 VCから退出しました。".to_string()
                    } else {
                        "⚠️ VC参加情報がありません。".to_string()
                    }
                }
                "say" => {
                    let guild_id = cmd.guild_id.unwrap();
                    let data = ctx.data.read().await;
                    let map = data.get::<BotState>().unwrap().clone();
                    if let Some(vc) = map.lock().await.get(&guild_id).cloned() {
                        let manager = songbird::get(&ctx).await.unwrap().clone();
                        if let Some(handler) = manager.get(guild_id) {
                            let text = cmd
                                .data
                                .options
                                .get(0)
                                .and_then(|o| o.value.as_ref())
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let url = build_tts_url(text);
                            match ffmpeg(url).await {
                                Ok(source) => {
                                    handler.lock().await.play_source(source);
                                    "💬 読み上げます。".to_string()
                                }
                                Err(_) => "❌ 音声取得に失敗しました。".to_string(),
                            }
                        } else {
                            "⚠️ ボイスチャネル接続情報がありません。".to_string()
                        }
                    } else {
                        "⚠️ 先に /join でVCに参加してください。".to_string()
                    }
                }
                _ => return,
            };
            let _ = cmd.create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                 .interaction_response_data(|m| m.content(content).ephemeral(true))
            }).await;
        }
    }

    // 旧 prefix 処理を残す場合のみ実装
    // async fn message(&self, ctx: Context, msg: Message) { ... }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKENが設定されていません");
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Client作成失敗");

    // 状態の初期化
    {
        let mut data = client.data.write().await;
        data.insert::<BotState>(Arc::new(Mutex::new(HashMap::new())));
    }

    if let Err(err) = client.start().await {
        eprintln!("❌ BOT 起動中にエラー: {:?}", err);
    }
}
