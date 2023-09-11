use anyhow::Result;
use tokio::{fs::File, io::AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<()>{
    let mut f = File::create("foo.txt").await?;
    
    f.write_all(b"Hello, world! This is result of AsyncWriteExt::write_all method.").await?;
    
    Ok(())
}