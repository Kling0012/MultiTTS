# MultiTTS

![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Kling0012/MultiTTS?utm_source=oss&utm_medium=github&utm_campaign=Kling0012%2FMultiTTS&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)

Rust 製の Discord ボイスチャットボットです。  
`serenity` と `songbird` を用いて、VC への参加と退出をスラッシュコマンドで制御できます。

## 依存クレート

- `serenity` – Discord API ライブラリ
- `songbird` – 音声再生ライブラリ
- `tokio` – 非同期ランタイム
- `dotenv` – 環境変数読み込み
- `reqwest`, `serde`, `serde_json`, `urlencoding` – 今後の機能拡張のために準備されています
- `whatlang` – 投稿メッセージの言語判定に使用

## 使い方

1. `.env` に以下の環境変数を設定します。
   ```env
   DISCORD_TOKEN=YOUR_TOKEN
   DISCORD_GUILD_ID=GUILD_ID
   ```
2. `cargo run` で BOT を起動します。
3. サーバー内で `/join` を実行するとボイスチャネルに参加し、`/leave` で退出します。
   `/say` コマンドでテキストを読み上げます。
   また、チャンネルに投稿されたメッセージが中国語の場合は自動で読み上げます。

TTS 機能として Google Translate の読み上げ音声を利用しています。
