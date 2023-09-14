use bytes::BytesMut;
use mini_redis::{Frame, Result};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub async fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            // 4KB のキャパシティをもつバッファを確保する
            buffer: BytesMut::with_capacity(4096),
        }
    }

    // ストリームからフレームを一つ読み込む
    // EOF であれば None を返す
    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            // バッファされたデータから単一のフレームをパースすることを試みて
            // うまくパースされたらフレームを返却する
            // 
            // もし Ok(None) が返ってきたら
            // - 追加の読み込みが必要
            // - EOF
            // - クライアント側で接続が切られた
            // のいずれかの状態なので、各々の場合に対応する処理を行う
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // ストリームからバッファに読み出しを行い、
            // 読み取った値の長さを取得
            // もし、新たに読み取ったバイト列の長さが 0 なら
            // - 読み込むべきフレームがないか（すなわち、EOF）
            // - クライアント側で接続が切られた
            // ことを表すので、各場合について処理
            //
            // もし、新たに読み取ったバイト列の長さが 0 以上なら
            // 次のループに移動して、バッファに読み込んだデータのパースを試みる
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                // EOF の場合は
                // バッファに中途半端にデータが残っていないはずなのでチェックする
                if self.buffer.is_empty() {
                    // もし、バッファも空なら
                    //「読み込むべきフレームがない（EOFである）」ことを伝えればよい
                    return Ok(None);
                } else {
                    // もし、まだバッファに中途半端にデータが残っているなら
                    // フレームの送信途中で、client 側から接続が切られたことを表す
                    // その旨を伝えるエラーを返す
                    return Err("Connection reset by peer".into());
                }
            }
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
