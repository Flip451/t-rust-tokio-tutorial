# framing: buffered writes

## 目次

- [framing: buffered writes](#framing-buffered-writes)
  - [目次](#目次)
  - [概要](#概要)

## 概要

- この節では、`write_frame(frame)` 関数を作成する
- この関数は、単一のフレーム全体をソケットに書き込む

- 今回の実装では、関数実行時に `write` システムコールの呼び出し回数を減らすために書き込みをバッファ化する

- つまり、ソケットにデータを書き込む際に、直接ソケットに書き込むのではなく、その前に、バッファにデータをため込んでから、一気にソケットにデータを流し込む

- これを実装するには、`BufWriter` 構造体を用いればよい
  - `AsyncWriter` トレイトを実装する値を引数にとって `BufWiriter::new` すれば、勝手にバッファリングを行ってくれる

  ```diff
  use std::io::Cursor;

  use bytes::{Buf, BytesMut};
  use mini_redis::frame::Error::Incomplete;
  use mini_redis::{Frame, Result};
  - use tokio::io::AsyncReadExt;
  + use tokio::io::{AsyncReadExt, BufWriter};
  use tokio::net::TcpStream;

  pub struct Connection {
  -   stream: TcpStream,
  +   stream: BufWriter<TcpStream>,
      buffer: BytesMut,
  }

  impl Connection {
      pub async fn new(stream: TcpStream) -> Self {
          Self {
  -           stream,
  +           // ただ BufWriter でラップするだけで良しなにバッファリングしてくれる
  +           stream: BufWriter::new(stream),
              // 4KB のキャパシティをもつバッファを確保する
              buffer: BytesMut::with_capacity(4096),
          }
      }

      // -- snip--
  }
  ```

- `write_frame` 関数本体では、`AsyncWriterExt` トレイトが提供するメソッドを利用して、ストリームへの書き込みを実装する

```rust
impl Connection {
    // --snip--

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
```
