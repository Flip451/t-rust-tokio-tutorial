# framing

## 目次

- [framing](#framing)
  - [目次](#目次)
  - [概要](#概要)
  - [Redis プロトコルのフレーム](#redis-プロトコルのフレーム)
  - [実装開始](#実装開始)
  - [読み取りのバッファ](#読み取りのバッファ)

## 概要

- この章では、mini-redis サーバーのフレーミング層を実装する
  - フレーミング層では、バイトストリームを受けとって、それをフレームのストリームに変換する
  - フレームとは、２つの peer 間で伝達されるデータの単位を表す

## Redis プロトコルのフレーム

```rust
use bytes::Bytes;

enum Frame {
    Simple(String),
    Error(String),
    Integer(u64),
    Bulk(Bytes),
    Null,
    Array(Vec<Frame>)
}
```

- フレームはいかなる意味も持たないデータだけで構成されていることに注意
- コマンドの解析および実装はもっと高いレイヤーで行われる

- cf. HTTP のフレーム

  ```rust
  enum {
      RequestHead {
          method: Method,
          uri: Uri,
          version: Version,
          headers: HeaderMap,
      },
      ResponseHead {
          status: StatusCode,
          version: Version,
          headers: HeaderMap,
      },
      BodyChunk {
          chunk: Bytes,
      },
  }
  ```

## 実装開始

- まず、Mini-Redis にフレーミングを実装するため、`TcpStream` をラップした構造体を作り、`mini_redis::Frame` の値を読み書きすることができるようにする

- そのはじまりとして、以下のように 構造体・enum を作成

  **`src/lib.rs`**

  ```rust
  mod connection;
  pub use connection::Connection;
  ```

  **`src/connection.rs`**

  ```rust
  use mini_redis::{Frame, Result};
  use tokio::net::TcpStream;

  pub struct Connection {
      stream: TcpStream,
  }

  impl Connection {
      // コネクションからフレームを読み込む
      // EOF に達したら None を返す
      pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
          todo!()
      }

      // コネクションにフレームを書き込む
      pub async fn write_frame(&mut self) -> Result<()> {
          todo!()
      }
  }
  ```

- redis のワイヤープロトコルについては <https://redis.io/docs/reference/protocol-spec/> を参照

## 読み取りのバッファ

- `read_frame` メソッドは、フレーム全体を受け取るまで待機してから値を返すような関数にする
  - ここで、`TcpStream::read` メソッドは、一回の呼び出しで、任意の大きさのデータを返すことに注意して実装する
    - このメソッドの返り値は、フレーム全体を含むかもしれないし、一部のみ含むかもしれないし、複数のフレームを含むかもしれない
    - これに対応するために、 `Connection` 構造体には、read 用のバッファフィールドを用意する必要がある
      - ソケットから読みだされたデータは、read バッファ に蓄えられて、フレームがパースされたら対応するデータがバッファから削除される
      - バッファの型としては `BytesMut` を用いる（`Bytes` の可変バージョン）

- まず、`Connection::new` を作成する

  ```diff
  + use bytes::BytesMut;
  use mini_redis::{Frame, Result};
  use tokio::net::TcpStream;

  pub struct Connection {
  -   stream: TcpStream
  +   stream: TcpStream,
  +   buffer: BytesMut,
  }

  impl Connection {
  +   pub async fn new(stream: TcpStream) -> Self {
  +       Self {
  +           stream,
  +           // 4KB のキャパシティをもつバッファを確保する
  +           buffer: BytesMut::with_capacity(4096),
  +       }
  +   }

      // コネクションからフレームを一つ読み込む
      // EOF であれば None を返す
      pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
          todo!()
      }

      // コネクションにフレームを書き込む
      pub async fn write_frame(&mut self) -> Result<()> {
          todo!()
      }
  }
  ```

- `read_frame` メソッドの実装

  ```diff
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

  +   // ストリームからフレームを一つ読み込む
  +   // EOF であれば None を返す
  +   pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
  +       loop {
  +           // バッファされたデータから単一のフレームをパースすることを試みて
  +           // うまくパースされたらフレームを返却する
  +           // 
  +           // もし Ok(None) が返ってきたら
  +           // - 追加の読み込みが必要
  +           // - EOF
  +           // - クライアント側で接続が切られた
  +           // のいずれかの状態なので、各々の場合に対応する処理を行う
  +           if let Some(frame) = self.parse_frame()? {
  +               return Ok(Some(frame));
  +           }
  + 
  +           // ストリームからバッファに読み出しを行い、
  +           // 読み取った値の長さを取得
  +           // もし、新たに読み取ったバイト列の長さが 0 なら
  +           // - 読み込むべきフレームがないか（すなわち、EOF）
  +           // - クライアント側で接続が切られた
  +           // ことを表すので、各場合について処理
  +           //
  +           // もし、新たに読み取ったバイト列の長さが 0 以上なら
  +           // 次のループに移動して、バッファに読み込んだデータのパースを試みる
  +           if 0 == self.stream.read_buf(&mut self.buffer).await? {
  +               // EOF の場合は
  +               // バッファに中途半端にデータが残っていないはずなのでチェックする
  +               if self.buffer.is_empty() {
  +                   // もし、バッファも空なら
  +                   //「読み込むべきフレームがない（EOFである）」ことを伝えればよい
  +                   return Ok(None);
  +               } else {
  +                   // もし、まだバッファに中途半端にデータが残っているなら
  +                   // フレームの送信途中で、client 側から接続が切られたことを表す
  +                   // その旨を伝えるエラーを返す
  +                   return Err("Connection reset by peer".into());
  +               }
  +           }
  +       }
  +   }

  +   // コネクションから単一のフレームをパースする関数
  +   // 1. 十分な量のデータがバッファされていたら、
  +   //    バッファからフレームを取り除いて、それを返却する
  +   // 2. フレームをパースするのに十分な量のデータがバッファされていなかったら
  +   //    Ok(None) を返却して、フレームを取り出せなかったが、
  +   //    追加でデータをバッファすれば問題ないはずであるか、
  +   //    読み込むべきデータがもうないかのいずれかであろうことを伝える
  +   // 3. 内部で問題が発生したら Err を返す
  +   fn parse_frame(&self) -> Result<Option<Frame>> {
  +       todo!()
  +   }

      // コネクションにフレームを書き込む
      pub async fn write_frame(&mut self) -> Result<()> {
          todo!()
      }
  }
  ```
