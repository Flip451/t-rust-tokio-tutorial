use tokio::{
    io,
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() -> io::Result<()> {
    // アドレスとポートをバインド
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        // インバウンドコネクションを受け付け
        let (mut socket, address) = listener.accept().await?;
        
        println!("Accept connection from {}", address);
        
        // 各コネクションごとにタスクをスポーン
        tokio::spawn(async move {
            // socket を read ハンドルと write ハンドルに分割
            let (mut rd, mut wr) = TcpStream::split(&mut socket);
            
            // read ハンドルを write ハンドルにコピー
            // 失敗したらエラーを表示
            if io::copy(&mut rd, &mut wr).await.is_err() {
                eprintln!("failed to copy");
            };
        });
    }
}
