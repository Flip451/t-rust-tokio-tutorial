use std::{collections::HashMap, sync::{Mutex, Arc}};

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};

use mini_redis::{Connection, Frame, Result};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main() -> Result<()> {
    // TCP 接続開始
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    let db = Db::new(Mutex::new(HashMap::new()));

    loop {
        // 接続を受け付け
        // 接続が実際に来るまでコードをブロック
        let (socket, address) = listener.accept().await?;
        let db = db.clone();

        // 接続元アドレスの表示
        println!("accept connection from {}", address);

        // リクエストの処理の実行
        // それぞれのインバウンドコネクションに対して新しい「タスク」をスポーン
        // ソケットをその「タスク」に move して利用する
        tokio::spawn(async move {
            let _ = process(socket, db).await;
        });
    }
}

// リクエストを処理する非同期関数
async fn process(socket: TcpStream, db: Db) -> Result<()> {
    use mini_redis::Command::{self, Get, Set};

    // mini-redis クレートで定義している `Connection` 構造体を用いることで、
    // バイト列ではなく Redis の「フレーム」を読み書き出来る
    let mut connection = Connection::new(socket);

    // 各コネクション内部で複数のコマンドを繰り返し受付できるように while ループを回す
    while let Some(frame) = connection.read_frame().await? {
        let response = match Command::from_frame(frame)? {
            Set(cmd) => {
                let mut db = db.lock().unwrap();
                // `Vec<u8> として保存する
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    // `Frame::Bulk` はデータが Bytes` 型であることを期待する
                    // この型についてはのちほど解説する
                    Frame::Bulk(value.clone())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplementd {:?}", cmd),
        };

        // クライアントへのレスポンスを書き込む
        connection.write_frame(&response).await?;
    }

    Ok(())
}