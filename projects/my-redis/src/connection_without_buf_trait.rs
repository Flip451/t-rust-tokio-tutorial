use bytes::BytesMut;
use mini_redis::{Frame, Result};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    // バッファを Vec<u8> に置き換える
    buffer: Vec<u8>,
    // バッファのどの位置までデータが書き込まれているかを
    // 記憶する cursor フィールドが追加で必要になる
    cursor: usize,
}

impl Connection {
    pub async fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            // 4KB のキャパシティをもつバッファを確保する
            buffer: Vec::with_capacity(4096),
            cursor: 0,
        }
    }

    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            // Buf トレイトの有無に関わらない部分はそのままでよい
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // 追加で、バッファが十分なキャパシティを持つように調整する処理が必要になる
            //     cursor の位置が、バッファの末端まで到達したら、
            //     バッファを拡張する
            if self.buffer.len() == self.cursor {
                self.buffer.resize(self.cursor * 2, 0);
            }

            // ストリームからデータをバッファに読みだす時には、
            // 既存のデータを上書きしないように、
            // cursor の位置を参考に、データの入っていない部分に書き込むよう注意する
            let n = self.stream.read(&mut self.buffer[self.cursor..]).await?;
            if 0 == n {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err("Connection reset by peer".into());
                }
            }

            // 追加で cursor の位置を更新する処理が必要になる
            self.cursor += n;
        }
    }

    // コネクションから単一のフレームをパースする関数
    // 1. 十分な量のデータがバッファされていたら、
    //    バッファからフレームを取り除いて、それを返却する
    // 2. フレームをパースするのに十分な量のデータがバッファされていなかったら
    //    Ok(None) を返却して、フレームを取り出せなかったが、
    //    追加でデータをバッファすれば問題ないはずであるか、
    //    読み込むべきデータがもうないかのいずれかであろうことを伝える
    // 3. 内部で問題が発生したら Err を返す
    fn parse_frame(&self) -> Result<Option<Frame>> {
        todo!()
    }

    // コネクションにフレームを書き込む
    pub async fn write_frame(&mut self) -> Result<()> {
        todo!()
    }
}
