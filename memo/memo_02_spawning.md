# spawning

## 目次

- [spawning](#spawning)
  - [目次](#目次)
  - [概要](#概要)
  - [準備](#準備)
  - [ソケットを受け付ける](#ソケットを受け付ける)
    - [並行性](#並行性)

## 概要

この節では、my-redis クレートの作成を開始する

## 準備

- 先の章で作成した redis サーバーへの書き込みと読み取りを連続実行するコードを `example` ディレクトリに移動する

  ```sh
  mkdir -p examples
  mv src/main.rs examples/hello-redis.rs
  ```

## ソケットを受け付ける

- まずはじめに、外部からの（インバウンドの）TCP コネクションを受け付ける
- 一旦、受け付けたリクエストの内容を標準出力に出力して、その内容の如何に関わらずエラーを返す実装をする：

  **`src/main.rs`**

  ```rust
  use tokio::net::{TcpListener, TcpStream};

  use mini_redis::{Connection, Frame, Result};

  #[tokio::main]
  async fn main() -> Result<()> {
      // TCP 接続開始
      let listener = TcpListener::bind("127.0.0.1:6379").await?;

      loop {
          // 接続を受け付け
          // 接続が実際に来るまでコードをブロック
          let (socket, address) = listener.accept().await?;
          // 接続元アドレスの表示
          println!("accept connection from {}", address);
          // リクエストの処理の実行
          process(socket).await?;
      }
  }

  // リクエストを処理する非同期関数
  async fn process(socket: TcpStream) -> Result<()> {
      // mini-redis クレートで定義している `Connection` 構造体を用いることで、
      // バイト列ではなく Redis の「フレーム」を読み書き出来る
      let mut connection = Connection::new(socket);

      // リクエスト内容を読み込む
      // 読み込み内容に従って処理を実行する
      if let Some(frame) = connection.read_frame().await? {
          // 受け取ったフレームを標準出力に表示
          println!("GOT: {:?}", frame);

          // エラーを返却
          // 現時点では未実装である旨を伝える
          let response = Frame::Error("unimplemented".to_string());
          connection.write_frame(&response).await?;
      }

      Ok(())
  }
  ```

- このコードを動かして、別のターミナルから `cargo run --example hello-redis` して、前章で作成した redis への書き込み・読み取りを行うプログラムを実行すると、以下のような出力が得られる

  - redis サーバー側：

    ```sh
    accept connection from 127.0.0.1:45508
    GOT: Array([Bulk(b"set"), Bulk(b"hello"), Bulk(b"world")])
    ```

    - クライアント側から送られてきた「フレーム」の内容がわかる
  
  - クライアント側：

    ```sh
    Error: "unimplemented"
    ```

    - サーバー側から、期待通りにエラーメッセージが返ってくる

### 並行性

- このサーバーの実装には、並行性に関する問題がある：
  - この実装では一度にひとつのインバウンドリクエストしか処理できない
  - つまり、一旦接続を受け付けると、レスポンスをソケットに書き込み終わるまでスレッドは別のリクエストを処理できない
- この問題を解消して、コネクションを並行で処理するために、ひとつひとつのインバウンドコネクションに対し、新しい「タスク」をスポーンし、コネクションをその「タスク」の中で処理する：

```diff
use tokio::net::{TcpListener, TcpStream};

use mini_redis::{Connection, Frame, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // TCP 接続開始
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    loop {
        // 接続を受け付け
        // 接続が実際に来るまでコードをブロック
        let (socket, address) = listener.accept().await?;
        
        // 接続元アドレスの表示
        println!("accept connection from {}", address);
        
        // リクエストの処理の実行
-       process(socket).await?;
+       // それぞれのインバウンドコネクションに対して新しい「タスク」をスポーン
+       // socket をその「タスク」に move して利用する
+       tokio::spawn (async move {
+           process(socket).await;
+       });
    }
}

// リクエストを処理する非同期関数
async fn process(socket: TcpStream) -> Result<()> {
    // --snip--
}
```
