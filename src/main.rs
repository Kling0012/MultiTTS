use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, id::{GuildId, ChannelId}},
    prelude::*,
};
use songbird::{SerenityInit, input::ffmpeg};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use dotenv::dotenv;
use std::env;
use urlencoding::encode;

/// イベントハンドラ本体
struct Handler;

/// ギルドごとのVC参加情報を保持するキー
struct BotState;
impl TypeMapKey for BotState {
    type Value = Arc<Mutex<HashMap<GuildId, ChannelId>>>;
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let guild_id = match msg.guild_id {
            Some(id) => id,
            None => return,
        };

        // VC参加マップを取得
        let data = ctx.data.read().await;
        let vc_map = data.get::<BotState>().unwrap().clone();
        let mut vc_map = vc_map.lock().await;

        // !join コマンド
        if msg.content == "!join" {
            if let Some(guild) = ctx.cache.guild(guild_id) {
                if let Some(vs) = guild.voice_states.get(&msg.author.id) {
                    if let Some(vc_chan) = vs.channel_id {
                        let manager = songbird::get(&ctx).await.unwrap().clone();
                        let _ = manager.join(guild_id, vc_chan).await;
                        vc_map.insert(guild_id, vc_chan);
                        let _ = msg.channel_id
                            .say(&ctx.http, "読み上げを開始します。")
                            .await;
                        return;
                    }
                }
            }
            let _ = msg.channel_id
                .say(&ctx.http, "VCに参加してから `!join` を送ってください。")
                .await;
        }
        // !leave コマンド
        else if msg.content == "!leave" {
            if vc_map.remove(&guild_id).is_some() {
                let manager = songbird::get(&ctx).await.unwrap().clone();
                let _ = manager.remove(guild_id).await;
                let _ = msg.channel_id
                    .say(&ctx.http, "VCから切断しました。")
                    .await;
            } else {
                let _ = msg.channel_id
                    .say(&ctx.http, "VC参加情報がありません。")
                    .await;
            }
        }
        // それ以外 → 読み上げ
        else if vc_map.contains_key(&guild_id) {
            // 環境変数 or デフォルト
            let addr = env::var("TTS_SERVER_URL").unwrap_or_else(|_| "127.0.0.1:5007".into());
            let url = format!(
                "http://{}/synthesize?text={}",
                addr,
                encode(&msg.content)
            );

            // ffmpeg に URL を渡してストリーミング再生
            let manager = songbird::get(&ctx).await.unwrap().clone();
            if let Some(handler_lock) = manager.get(guild_id) {
                let mut handler = handler_lock.lock().await;
                match ffmpeg(&url).await {
                    Ok(source) => {
                        // unit 型のアームに統一
                        handler.play_source(source);
                    }
                    Err(e) => {
                        let _ = msg.channel_id
                            .say(&ctx.http, format!("❌ 音声再生に失敗しました: {}", e))
                            .await;
                    }
                }
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("✅ ログイン成功: {}", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("DISCORD_TOKEN が設定されていません");

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::Client::builder(&token, intents)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Client作成失敗");

    {
        let mut data = client.data.write().await;
        data.insert::<BotState>(Arc::new(Mutex::new(HashMap::new())));
    }

    if let Err(err) = client.start().await {
        eprintln!("❌ BOT 起動中にエラー: {:?}", err);
    }
}
