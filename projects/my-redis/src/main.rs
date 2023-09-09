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
        tokio::spawn (async move {
            process(socket).await;
        });
    }
}

// リクエストを処理する非同期関数
async fn process(socket: TcpStream) -> Result<()> {
    // mini-redis クレートで定義している `Connection` 構造体を用いることで、
    // バイト列ではなく Redis の「フレーム」を読み書き出来る
    let mut connection = Connection::new(socket);

    // リクエスト内容を読み込む
    // 読み込み内容に従って処理を実行する
    if let Some(frame) = connection.read_frame().await? {
        // 受け取ったフレームを標準出力に表示
        println!("GOT: {:?}", frame);

        // エラーを返却
        // 現時点では未実装である旨を伝える
        let response = Frame::Error("unimplemented".to_string());
        connection.write_frame(&response).await?;
    }

    Ok(())
}