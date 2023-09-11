use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    // サーバーに接続
    let socket = TcpStream::connect("127.0.0.1:6142").await?;

    // reader と writer に分割
    let (mut rd, mut wr) = io::split(socket);

    // バックグラウンドでデータを書き込み
    let write_task = tokio::spawn(async move {
        wr.write_all(b"Hello, ").await?;
        wr.write_all(b"world.\r\n").await?;
        Ok::<_, io::Error>(())
    });

    let mut buf = vec![0; 128];
    loop {
        match rd.read(&mut buf).await? {
            0 => break,
            n => {
                println!("GOT: {:?}", String::from_utf8(buf[..n].to_vec()))
            }
        }
    }

    Ok(())
}
