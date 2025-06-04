use serenity::{
    async_trait,
    model::{
        gateway::Ready,
        id::GuildId,
        prelude::*,
    },
    prelude::*,
};
use songbird::{SerenityInit, input::ffmpeg};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use dotenv::dotenv;
use std::env;
use urlencoding::encode;
use whatlang::{detect, Lang};



fn build_tts_url(text: &str, lang: &str) -> String {
    format!(
        "https://translate.google.com/translate_tts?ie=UTF-8&q={}&tl={}&client=tw-ob",
        encode(text),
        lang
    )
}

fn is_chinese(text: &str) -> bool {
    detect(text)
        .map(|info| matches!(info.lang(), Lang::Cmn))
        .unwrap_or(false)
}

#[derive(Clone)]
struct ChannelInfo {
    #[allow(dead_code)]
    voice: serenity::model::id::ChannelId,
    text: serenity::model::id::ChannelId,
}

struct BotState;
impl TypeMapKey for BotState {
    type Value = Arc<Mutex<HashMap<GuildId, ChannelInfo>>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("✅ ログイン成功: {}", ready.user.name);
        
        // グローバルスラッシュコマンドを登録（全てのギルドで使用可能）
        serenity::model::application::command::Command::set_global_application_commands(&ctx.http, |commands| {
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
        .expect("グローバルコマンド登録失敗");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(cmd) = interaction {
            let content = match cmd.data.name.as_str() {
                "join" => {
                    let guild_id = cmd.guild_id.unwrap();
                    if let Some(member) = cmd.member.clone() {
                        // ギルドからボイス状態を取得
                        let guild = guild_id.to_guild_cached(&ctx.cache).unwrap();
                        if let Some(voice_state) = guild.voice_states.get(&member.user.id) {
                            if let Some(vc) = voice_state.channel_id {
                                let manager = songbird::get(&ctx).await.unwrap().clone();
                                let _ = manager.join(guild_id, vc).await;
                                // 状態に保存
                                let data = ctx.data.read().await;
                                let map = data.get::<BotState>().unwrap().clone();
                                map.lock().await.insert(
                                    guild_id,
                                    ChannelInfo {
                                        voice: vc,
                                        text: cmd.channel_id,
                                    },
                                );
                                "✅ ボイスチャネルに参加しました。".to_string()
                            } else {
                                "⚠️ まずVCに参加してください。".to_string()
                            }
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
                    let info = map.lock().await.get(&guild_id).cloned();
                    drop(data);  // データへの参照を早めに削除
                    
                    if let Some(_info) = info {
                        let manager = songbird::get(&ctx).await.unwrap().clone();
                        if let Some(handler) = manager.get(guild_id) {
                            let text = cmd
                                .data
                                .options
                                .get(0)
                                .and_then(|o| o.value.as_ref())
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let url = build_tts_url(text, "ja");
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

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        if let Some(guild_id) = msg.guild_id {
            let data = ctx.data.read().await;
            let map = data.get::<BotState>().unwrap().clone();
            let info = map.lock().await.get(&guild_id).cloned();
            drop(data);  // データへの参照を早めに削除
            
            if let Some(info) = info {
                if info.text == msg.channel_id {
                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    if let Some(handler) = manager.get(guild_id) {
                        let lang = if is_chinese(&msg.content) { "zh-CN" } else { "ja" };
                        let url = build_tts_url(&msg.content, lang);
                        if let Ok(source) = ffmpeg(url).await {
                            handler.lock().await.play_source(source);
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKENが設定されていません");
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
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
