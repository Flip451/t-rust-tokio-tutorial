use anyhow::{Result, Ok};
use tokio::{fs::File, io::AsyncReadExt};

#[tokio::main]
async fn main() -> Result<()>{
    let mut f = File::open("foo.txt").await?;
    let mut buffer = vec![];
    
    let n = f.read_to_end(&mut buffer).await?;
    println!("The bytes: {:?}, size: {}", buffer, n);
    Ok(())
}