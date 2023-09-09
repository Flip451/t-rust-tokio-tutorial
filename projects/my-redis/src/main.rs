use tokio::net::{TcpListener, TcpStream};

use mini_redis::{Connection, Frame, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // TCP 接続開始
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    loop {
        // 接続を受け付け
        // 接続が実際に来るまでコードをブロック
        let (socket, address) = listener.accept().await?;

        // 接続元アドレスの表示
        println!("accept connection from {}", address);

        // リクエストの処理の実行
        // それぞれのインバウンドコネクションに対して新しい「タスク」をスポーン
        // ソケットをその「タスク」に move して利用する
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}

// リクエストを処理する非同期関数
async fn process(socket: TcpStream) -> Result<()> {
    use mini_redis::Command::{self, Get, Set};
    use std::collections::HashMap;

    // この実装では、一旦各コネクションごとにひとつずつ db が作成される
    // つまり、あるコネクションの内部では db の変更が保持・反映されるが、
    // 他のコネクションから、この変更にアクセスすることはできない
    let mut db = HashMap::new();

    // mini-redis クレートで定義している `Connection` 構造体を用いることで、
    // バイト列ではなく Redis の「フレーム」を読み書き出来る
    let mut connection = Connection::new(socket);

    // 各コネクション内部で複数のコマンドを繰り返し受付できるように while ループを回す
    while let Some(frame) = connection.read_frame().await? {
        let response = match Command::from_frame(frame)? {
            Set(cmd) => {
                // `Vec<u8> として保存する
                db.insert(cmd.key().to_string(), cmd.value().to_vec());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                if let Some(value) = db.get(cmd.key()) {
                    // `Frame::Bulk` はデータが Bytes` 型であることを期待する
                    // この型についてはのちほど解説する
                    Frame::Bulk(value.clone().into())
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