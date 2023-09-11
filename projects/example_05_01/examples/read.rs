use anyhow::Result;
use tokio::{fs::File, io::AsyncReadExt};

#[tokio::main]
async fn main() -> Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = [0; 10];

    // 最大 10 バイト読み込む
    let n = f.read(&mut buffer).await?;
    println!("The bytes: {:?}", &buffer[..n]);
    Ok(())
}
