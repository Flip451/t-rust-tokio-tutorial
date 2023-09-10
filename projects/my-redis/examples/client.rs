use bytes::Bytes;
use mini_redis::{client, Result};
use tokio::sync::mpsc;

#[derive(Debug)]
enum Command {
    Get { key: String },
    Set { key: String, val: Bytes },
}

#[tokio::main]
async fn main() -> Result<()> {
    // 最大で 32 のキャパシティを持つチャンネルを作成
    let (tx, mut rx) = mpsc::channel::<Command>(32);

    let manager = tokio::spawn(async move {
        // TCP コネクションを確立
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        // rx.recv().await の結果が None のときは
        // すべての送信機がドロップしているので
        // コマンドを受け付ける必要がなくなる
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key } => {
                    let _ = client.get(&key).await;
                }
                Command::Set { key, val } => {
                    let _ = client.set(&key, val).await;
                }
            }
        }
    });

    // 2つのタスクを spawn する。
    // タスク1 はキーによる "get" を行い、
    // タスク2 は値を "set" する。
    let tx1 = tx.clone();
    let t1 = tokio::spawn(async move {
        // レシーバに GET コマンドを送信する
        let _ = tx1
            .send(Command::Get {
                key: "hello".to_string(),
            })
            .await;
    });

    let tx2 = tx.clone();
    let t2 = tokio::spawn(async move {
        // レシーバに SET コマンドを送信する
        let _ = tx2
            .send(Command::Set {
                key: "foo".to_string(),
                val: "bar".into(),
            })
            .await;
    });

    // プロセスが終了してしまう前に
    // コマンドが完了したことを保証するため
    // main 関数の一番下で join ハンドルを .await
    t1.await?;
    t2.await?;
    manager.await?;
    Ok(())
}
