use std::io::{Cursor, self};

use bytes::{Buf, BytesMut};
use mini_redis::frame::Error::Incomplete;
use mini_redis::{Frame, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;

pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    pub async fn new(stream: TcpStream) -> Self {
        Self {
            // ただ BufWriter でラップするだけで良しなにバッファリングしてくれる
            stream: BufWriter::new(stream),
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
    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        // Frame 構造体はパースの実行のためにカーソルを用いる
        let mut buf = Cursor::new(&self.buffer[..]);

        // まず、単一のフレームをパースするのに十分なデータがバッファに蓄えられているかをチェックする
        // その結果によって場合分けする：
        //      もし、十分な量蓄えられていたら、Ok(_) のアームに進む
        //      不十分なら Err(Incomplete) のアームに進む
        //      それ以外のエラーが発生しているようならエラーを返す
        match Frame::check(&mut buf) {
            Ok(_) => {
                // parse に成功した場合、`Frame::check` 関数は
                // カーソルの位置をフレームの終端にまで進める
                // なので、カーソル位置を取得するとフレームの長さがわかる
                let len = buf.position() as usize;

                // パースの実行のためにカーソル位置を先頭に戻す
                buf.set_position(0);

                // フレームを取得する
                // もし、エラーが返ってきたら、送られてきたフレームの内容が不正であることを表すので、
                // このコネクションを切断する
                let frame = Frame::parse(&mut buf)?;

                // バッファされているデータのうち、パースし終えた部分を破棄する
                //      advance(cnt) が読み込み用のバッファに対して呼ばれると、
                //      `cnt` までのデータがすべて破棄される
                self.buffer.advance(len);

                // パースに成功したフレームを返す
                Ok(Some(frame))
            }
            // 十分な量のデータがバッファされていなかった場合
            Err(Incomplete) => Ok(None),
            // エラーが発生した場合
            Err(e) => Err(e.into()),
        }
    }

    // コネクションにフレームを書き込む
    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Simple(val) => {
                self.stream.write_u8(b'+').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                self.stream.write_u8(b'-').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                self.stream.write_u8(b':').await?;
                self.write_decimal(*val).await?;
            }
            Frame::Null => {
                self.stream.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();

                self.stream.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.stream.write_all(val).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Array(_val) => unimplemented!(),
        }

        // BufWriter は中間バッファに書き込みを蓄えるので
        // write を呼び出してもデータがソケットへと書き込まれることは保証されない
        // そこで return する前にフレームがソケットへと書き込まれるように
        // flush() を呼んびだして、バッファの中で保留状態となっているデータをすべてソケットへと書き込む
        self.stream.flush().await
    }

    /// Write a decimal frame to the stream
    async fn write_decimal(&mut self, val: u64) -> io::Result<()> {
        use std::io::Write;

        // Convert the value to a string
        let mut buf = [0u8; 20];
        let mut buf = Cursor::new(&mut buf[..]);
        write!(&mut buf, "{}", val)?;

        let pos = buf.position() as usize;
        self.stream.write_all(&buf.get_ref()[..pos]).await?;
        self.stream.write_all(b"\r\n").await?;

        Ok(())
    }
}
