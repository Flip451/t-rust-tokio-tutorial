use mini_redis::{client, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // サーバーとの接続の確立
    let mut client = client::connect("127.0.0.1:6379").await?;

    // 2つのタスクを spawn する。
    // タスク1 はキーによる "get" を行い、// タスク2 は値を "set" する。
    let t1 = tokio::spawn(async move {
        let _res = client.get("hello").await;
    });

    let t2 = tokio::spawn(async move {
        let _res = client.set("foo", "bar".into()).await;
    });

    t1.await?;
    t2.await?;
    Ok(())
}
