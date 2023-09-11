use std::vec;

use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::TcpListener};

#[tokio::main]
async fn main() -> io::Result<()> {
    // アドレスとポートをバインド
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        // クライアントからのコネクションを受け付け
        let (mut socket, address) = listener.accept().await?;
        println!("Accept connection from {}", address);

        // 各コネクションごとにタスクをスポーン
        tokio::spawn(async move {
            // 通常の配列ではなく vec! を用いることで
            // バッファをスタック上に配置するのを明示的に回避
            // これにより、メモリ上のタスクのサイズを軽減することができる
            let mut buf = vec![0; 1024];
            
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if socket.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    },
                    Err(_) => break,
                } 
            }
        });
    }
}