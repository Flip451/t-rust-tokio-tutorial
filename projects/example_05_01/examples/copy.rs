use anyhow::Result;
use tokio::{fs::File, io};

#[tokio::main]
async fn main() -> Result<()>{
    // &[u8] は AsyncReader トレイトを実装している
    let mut reader: &[u8] = b"hello";
    let mut f = File::create("foo.txt").await?;

    // ファイルに `reader` の内容を書き写すことができる
    // pub async fn copy<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> io::Result<u64>
    // where
    // R: AsyncRead + Unpin + ?Sized,
    // W: AsyncWrite + Unpin + ?Sized,
    io::copy(&mut reader, &mut f).await?;

    Ok(())
}