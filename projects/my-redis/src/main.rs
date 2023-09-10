use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};

use mini_redis::{Connection, Frame, Result};

type Db = Mutex<HashMap<String, Bytes>>;
type ShardedDb = Arc<Vec<Mutex<HashMap<String, Bytes>>>>;

// シャーディングされた db を作成する関数
fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut db = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        let m = Mutex::new(HashMap::new());
        db.push(m);
    }
    Arc::new(db)
}

// シャーディングされた db の中から該当の db を拾い上げる関数
fn get_db_from_sharded_db<'a>(shaded_db: &'a ShardedDb, key: &'a str) -> &'a Db {
    let db = &shaded_db[hash(key) % shaded_db.len()];
    db
}

// ハッシュ化関数
fn hash(key: &str) -> usize {
    let mut s = DefaultHasher::new();
    key.hash(&mut s);
    s.finish() as usize
}

#[tokio::main]
async fn main() -> Result<()> {
    // TCP 接続開始
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    let db = new_sharded_db(5);

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
async fn process(socket: TcpStream, db: ShardedDb) -> Result<()> {
    use mini_redis::Command::{self, Get, Set};

    // mini-redis クレートで定義している `Connection` 構造体を用いることで、
    // バイト列ではなく Redis の「フレーム」を読み書き出来る
    let mut connection = Connection::new(socket);

    // 各コネクション内部で複数のコマンドを繰り返し受付できるように while ループを回す
    while let Some(frame) = connection.read_frame().await? {
        let response = match Command::from_frame(frame)? {
            Set(cmd) => {
                let db = get_db_from_sharded_db(&db, cmd.key());
                let mut db = db.lock().unwrap();
                // `Vec<u8> として保存する
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let db = get_db_from_sharded_db(&db, cmd.key());
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
