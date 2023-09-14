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
```