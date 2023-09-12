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
  - フレーミング層では、バイトストリームを受けっとて、それをフレームのストリームに変換する
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

- フレームはいかなる意味をも持たないデータだけで構成されていることに注意
- コマンドの解析および実装はもっと高いレイヤーで行われる

cf. HTTP のフレーム

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

- まず、Mini-Redis にフレーミングを実装するため、`TcpStream` をラップした構造体を作り、`mini_redis::Frame`` の値を読み書きすることができるようにする

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
      // コネクションからフレームを読み込み
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
  +   buffer: BytesMut
  }

  impl Connection {
  +   pub async fn new(stream: TcpStream) -> Self {
  +       Self {
  +           stream,
  +           // 4KB のキャパシティをもつバッファを確保する
  +           buffer: BytesMut::with_capacity(4096)
  +       }
  +   }

      // コネクションからフレームを読み込み
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

- `read` メソッドの実装

```rust

```