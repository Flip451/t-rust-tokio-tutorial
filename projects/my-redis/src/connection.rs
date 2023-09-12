use bytes::BytesMut;
use mini_redis::{Frame, Result};
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut
}

impl Connection {
    pub async fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            // 4KB のキャパシティをもつバッファを確保する
            buffer: BytesMut::with_capacity(4096)
        }
    }

    // コネクションからフレームを読み込み
    // EOF に達したら None を返す
    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            // バッファされたデータからフレームをパースすることを試みて
            // うまくパースされたらフレームを返却する
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // 
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err("Connection reset by peer".into());
                }
            }
        }
    }

    // コネクションにフレームを書き込む
    pub async fn write_frame(&mut self) -> Result<()> {
        todo!()
    }

    // バッファからフレームをパースする関数
    fn parse_frame(&self) -> Result<Option<Frame>> {
        todo!()
    }
}
