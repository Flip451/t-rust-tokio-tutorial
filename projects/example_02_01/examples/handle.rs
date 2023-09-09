use anyhow::{Result, Ok};

#[tokio::main]
async fn main() -> Result<()>{
    let handle = tokio::spawn(async {
        "return value"
    });

    let out = handle.await?;
    println!("{}", out);
    Ok(())
}
