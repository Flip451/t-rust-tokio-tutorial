# I/O エコーサーバの作成

## 目次

- [I/O エコーサーバの作成](#io-エコーサーバの作成)
  - [目次](#目次)
  - [概要](#概要)
  - [`io::copy` を使う場合](#iocopy-を使う場合)
    - [`io::split` の使用例](#iosplit-の使用例)
  - [手動でコピーする場合](#手動でコピーする場合)

## 概要

- 非同期 I/O を使う練習として、Echo サーバを作成する
- このサーバでは
  - TCP 接続をバインドして、
  - ループの中でインバウンドコネクションを受け付ける
  - 一回一回のインバウンドコネクションに対して、ソケットからデータを読み込んですぐにソケットに書き込む
- このような Echo サーバを少し異なる方法で二つ作成する

## `io::copy` を使う場合

- 以下のコードはコンパイルエラーを起こす：

  ```rust
  use tokio::{
      io,
      net::{TcpListener, TcpStream},
  };

  #[tokio::main]
  async fn main() -> io::Result<()> {
      // アドレスとポートをバインド
      let listener = TcpListener::bind("127.0.0.1:6142").await?;

      loop {
          // インバウンドコネクションを受け付け
          let (mut socket, address) = listener.accept().await?;
          
          println!("Accept connection from {}", address);
          
          // 各コネクションごとにタスクをスポーン
          tokio::spawn(async move {
              // socket を read ハンドルと write ハンドルに分割
              let (mut rd, mut wr) = TcpStream::split(&mut socket);
              
              // read ハンドルを write ハンドルにコピー
              // 失敗したらエラーを表示
              if io::copy(&mut rd, &mut wr).await.is_err() {
                  eprintln!("failed to copy");
              };
          });
      }
  }
  ```

  - なぜならば、`socket` の可変参照を同一スコープ内で二つ生成しているから

- この問題を回避しながらリクエストをレスポンスにコピーするには、ソケットを reader ハンドルと writer ハンドルへと分割する

- 任意の `AsyncReader + AsyncWriter` 型は、`io::split` で reader ハンドルと writer ハンドルに分割することができ、これらのハンドルは独立に利用できる

- しかし、`io::split` は内部的に `Arc` と `Mutex` を使って実装されており、これに伴うオーバーヘッドがある

- `TcpStream` の場合は、`TcpStream::split` を用いることで、このオーバーヘッドを回避できる
  - この関数は、ストリームへの**参照**を受け取って reader ハンドルと writer ハンドルを返す
  - ただし、あくまで受け取るのは参照なので、別のタスクにムーブすることはできない点に注意
  - この `split` はゼロコスト
- また、`TcpStream` は `into_split` という関数も提供する
  - この関数は、タスクをまたいでムーブ可能なハンドルを生成できる
  - ただし、こちらは内部的に `Arc` を使うコストは必要になる

- `TcpStream::split` を用いて上記の問題を解消したコードは以下の通り：

  ```rust
  use tokio::{
      io,
      net::{TcpListener, TcpStream},
  };

  #[tokio::main]
  async fn main() -> io::Result<()> {
      // アドレスとポートをバインド
      let listener = TcpListener::bind("127.0.0.1:6142").await?;

      loop {
          // インバウンドコネクションを受け付け
          let (mut socket, address) = listener.accept().await?;
          let (mut rd, mut wr) = TcpStream::split(&mut socket);

          println!("Accept connection from {}", address);

          io::copy(&mut rd, &mut wr).await?;
      }
  }
  ```

### `io::split` の使用例

- Echo サーバのクライアントを実装する際に、 `io::split` を用いることで、echo クライアントは、読み込みと書き込みを並行で処理できる：

  ```rust
  use tokio::{
      io::{self, AsyncReadExt, AsyncWriteExt},
      net::TcpStream,
  };

  #[tokio::main]
  async fn main() -> io::Result<()> {
      // サーバーに接続
      let socket = TcpStream::connect("127.0.0.1:6142").await?;

      // reader と writer に分割
      let (mut rd, mut wr) = io::split(socket);

      // バックグラウンドでデータを書き込み
      let write_task = tokio::spawn(async move {
          wr.write_all(b"Hello, ").await?;
          wr.write_all(b"world.\r\n").await?;
          Ok::<_, io::Error>(())
      });

      let mut buf = vec![0; 128];
      loop {
          match rd.read(&mut buf).await? {
              0 => break,
              n => {
                  println!("GOT: {:?}", String::from_utf8(buf[..n].to_vec()))
              }
          }
      }

      Ok(())
  }
  ```

## 手動でコピーする場合

- あえて、`io::copy` を用いずに手動でコピーして echo サーバーを作成すると以下のようになる：

  ```rust
  use std::vec;

  use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::TcpListener};

  #[tokio::main]
  async fn main() -> io::Result<()> {
      // アドレスとポートをバインド
      let listener = TcpListener::bind("127.0.0.1:6142").await?;

      loop {
          // クライアントからのコネクションを受け付け
          let (mut socket, address) = listener.accept().await?;
          println!("Accept connection from {}", address);

          // 各コネクションごとにタスクをスポーン
          tokio::spawn(async move {
              // 通常の配列ではなく vec! を用いることで
              // バッファをスタック上に配置するのを明示的に回避
              // これにより、メモリ上のタスクのサイズを軽減することができる
              let mut buf = vec![0; 1024];
              
              loop {
                  match socket.read(&mut buf).await {
                      Ok(0) => break,
                      Ok(n) => {
                          if socket.write_all(&buf[..n]).await.is_err() {
                              break;
                          }
                      },
                      Err(_) => break,
                  } 
              }
          });
      }
  }
  ```

- ただし、ここでは、バッファの確保を `[u8; 1024]` （スタック上に確保される配列型）ではなく `Vec<u8>` （ヒープ上に確保されるベクタ）で行っていることに注意

  - 実は、こちらの方が効率が良い
  - その理由として、以下の2つが挙げられる。

    1. スタック配列だと `.await` をまたぐときにスレッド間で全体のムーブが必要になる（ベクタであればヒープへのポインタ + 補助的な僅かなデータをムーブするだけで済む）

    2. スタック配列の場合、タスクを表す構造体がバッファデータをすべて含むことになるため、構造体サイズが大きくなってしまう（ベクタであればヒープへのポインタ + 補助的な僅かなデータ を含むだけでよく、軽量に保てる）

- また、`socket.read(...).await` の結果が `Ok(0)` になったときに読み取りのループを終えることを忘れると、CPU が無限ループを起こしてしまうので注意