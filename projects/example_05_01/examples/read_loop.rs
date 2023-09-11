use anyhow::Result;
use tokio::{fs::File, io::AsyncReadExt};

#[tokio::main]
async fn main() -> Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = [0; 4];

    loop {
        match f.read(&mut buffer).await? {
            0 => break,
            n => {
                println!("{}", String::from_utf8(buffer[..n].to_vec()).unwrap())
            }
        }
    }

    Ok(())
}
