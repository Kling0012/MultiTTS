use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, id::{GuildId, ChannelId}},
    prelude::*,
};
use songbird::{SerenityInit, input::ffmpeg};
use std::{collections::HashMap, sync::Arc};
use tts::Tts;
use tokio::sync::Mutex;
use dotenv::dotenv;
use std::env;

/// BOTがVC参加しているギルドとチャンネルの対応を記録
struct BotState {
    vc_map: Arc<Mutex<HashMap<GuildId, ChannelId>>>,
}

impl TypeMapKey for BotState {
    type Value = Arc<Mutex<HashMap<GuildId, ChannelId>>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // BOT自身のメッセージは無視
        if msg.author.bot {
            return;
        }

        let guild_id = match msg.guild_id {
            Some(id) => id,
            None => return,
        };

        let data = ctx.data.read().await;
        let vc_map_lock = data.get::<BotState>().unwrap().clone();
        let mut vc_map = vc_map_lock.lock().await;

        if msg.content == "!join" {
            // 呼び出し主がいるVCを取得
            if let Some(voice_state) = msg.author.voice_state(&ctx.cache).await {
                if let Some(vc_channel_id) = voice_state.channel_id {
                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    let _ = manager.join(guild_id, vc_channel_id).await;

                    // 状態を記録
                    vc_map.insert(guild_id, vc_channel_id);

                    let _ = msg.channel_id.say(&ctx.http, "読み上げを開始します。").await;
                }
            }

        } else if msg.content == "!leave" {
            if let Some(vc_channel_id) = vc_map.remove(&guild_id) {
                let manager = songbird::get(&ctx).await.unwrap().clone();
                let _ = manager.remove(guild_id).await;

                let _ = msg.channel_id.say(&ctx.http, "VCから切断しました。").await;
            }

        } else {
            // 通常の発言メッセージ：読み上げ対象
            if let Some(vc_channel_id) = vc_map.get(&guild_id) {
                // TTSで音声ファイル生成
                let tts = Tts::default().expect("TTS 初期化失敗");
                tts.speak_to_file(&msg.content, "output.wav").expect("音声生成失敗");

                // 音声をVCに再生
                let manager = songbird::get(&ctx).await.unwrap().clone();
                if let Some(handler_lock) = manager.get(guild_id) {
                    let mut handler = handler_lock.lock().await;
                    let source = ffmpeg("output.wav").await.expect("FFmpeg読み込み失敗");
                    handler.enqueue_source(source);
                }
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} としてログインしました。", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // .env ファイルの読み込み
    dotenv().ok();

    // 環境変数 DISCORD_TOKEN を取得
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKENが環境変数に設定されていません");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Client作成失敗");

    // BOTの共有データにVC状態マップを追加
    {
        let mut data = client.data.write().await;
        data.insert::<BotState>(Arc::new(Mutex::new(HashMap::new())));
    }

    if let Err(why) = client.start().await {
        println!("エラー: {:?}", why);
    }
}
