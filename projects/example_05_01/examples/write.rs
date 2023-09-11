use anyhow::Result;
use tokio::{fs::File, io::AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<()> {
    // `File::open` ではなく `File::create` を使うと、
    // 書き込み専用モードでファイルを開ける
    let mut f = File::create("foo.txt").await?;

    let bytes = b"Hello, world!
This is from async rust.
";

    // バイト列の先頭からいくつかを書き込む
    // 必ずしもすべてを書き込むわけではない
    let n = f.write(bytes).await?;

    println!(
        "Wrote the first {:?} bytes of {:?}",
        n,
        String::from_utf8(bytes.to_vec())
    );
    Ok(())
}
