use bytes::Bytes;
use mini_redis::{client, Result};
use tokio::sync::{mpsc, oneshot};

// resp の型は、"マネージャー" タスクの
// client.get(...).await, client.set(...).await
// の返り値の型を基準に決定
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}

type Responder<T> = oneshot::Sender<Result<T>>;

#[tokio::main]
async fn main() -> Result<()> {
    // 最大で 32 のキャパシティを持つチャンネルを作成
    let (tx, mut rx) = mpsc::channel::<Command>(32);

    let manager = tokio::spawn(async move {
        // Redis サーバとの間の TCP コネクションを確立
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        // 各タスク（t1, t2）からのリクエストを受信
        //     rx.recv().await の結果が None のときは
        //     すべての送信機がドロップしているので
        //     コマンドを受け付ける必要がなくなる
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key, resp } => {
                    // Redis サーバとの通信
                    let res: Result<Option<Bytes>> = client.get(&key).await;

                    // Redis サーバとの通信結果を返却
                    //     なお、send メソッドがエラーをはいた場合は無視する
                    //     それは受信側（元のタスク）の受信機がすでにドロップされており
                    //     受信側にレスポンスへの興味がすでにないことを表すから
                    let _ = resp.send(res);
                }
                Command::Set { key, val, resp } => {
                    // Redis サーバとの通信
                    let res: Result<()> = client.set(&key, val).await;

                    // Redis サーバとの通信結果を返却
                    //     なお、send メソッドがエラーをはいた場合は無視する
                    //     それは受信側（元のタスク）の受信機がすでにドロップされており
                    //     受信側にレスポンスへの興味がすでにないことを表すから
                    let _ = resp.send(res);
                }
            }
        }
    });

    // 2つのタスクを spawn する。
    // タスク1 はキーによる "get" を行い、
    // タスク2 は値を "set" する。
    let tx1 = tx.clone();
    let t1 = tokio::spawn(async move {
        // "マネージャー"タスクから Redis サーバとの通信結果を受け取るためのチャンネルを作成
        let (resp_tx, resp_rx) = oneshot::channel();

        // マネージャータスク内のレシーバに GET コマンドを送信する
        // 送信が失敗したときにプログラムが終了するように unwrap する
        // コマンドと一緒に、マネージャータスクが通信結果を送り返すための送信機も添える
        let _ = tx1
            .send(Command::Get {
                key: "hello".to_string(),
                resp: resp_tx,
            })
            .await
            .unwrap();

        // マネージャータスクから、Redis の通信結果が返ってくるのを待つ
        let response = resp_rx.await;
        println!("GOT: {:?}", response)
    });

    let tx2 = tx;
    let t2 = tokio::spawn(async move {
        // "マネージャー"タスクから Redis サーバとの通信結果を受け取るためのチャンネルを作成
        let (resp_tx, resp_rx) = oneshot::channel();

        // レシーバに SET コマンドを送信する
        // 送信が失敗したときにプログラムが終了するように unwrap する
        // コマンドと一緒に、マネージャータスクが通信結果を送り返すための送信機も添える
        let _ = tx2
            .send(Command::Set {
                key: "foo".to_string(),
                val: "bar".into(),
                resp: resp_tx,
            })
            .await
            .unwrap();

        // マネージャータスクから、Redis の通信結果が返ってくるのを待つ
        let response = resp_rx.await;
        println!("GOT: {:?}", response);
    });

    // プロセスが終了してしまう前に
    // コマンドが完了したことを保証するため
    // main 関数の一番下で join ハンドルを .await
    t1.await?;
    t2.await?;
    manager.await?;
    Ok(())
}
