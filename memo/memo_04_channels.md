# チャンネル

## 目次

- [チャンネル](#チャンネル)
  - [目次](#目次)
  - [概要](#概要)
  - [愚直な実装（失敗例）](#愚直な実装失敗例)
  - [メッセージ受け渡し](#メッセージ受け渡し)
  - [Tokio のチャンネルプリミティブ](#tokio-のチャンネルプリミティブ)
  - [メッセージ型の定義](#メッセージ型の定義)
  - [チャンネルを作成](#チャンネルを作成)
  - ["マネージャー" タスクをスポーンする](#マネージャー-タスクをスポーンする)
  - [レスポンスを受け取る](#レスポンスを受け取る)

## 概要

- 続いて、これまでに学んできたことをクライアントサイドにも適用する
- この章では、２へ移行で Redis コマンドを実行する
  - コマンド一つにつき一つのタスクをスポーンして、二つのコマンドを平行に処理する

## 愚直な実装（失敗例）

- まず、以下のように実装を試みる：

  ```rust
  use mini_redis::{client, Result};

  #[tokio::main]
  async fn main() -> Result<()> {
      // サーバーとの接続の確立
      let mut client = client::connect("127.0.0.1:6379").await?;

      // 2つのタスクを spawn する。
      // タスク1 はキーによる "get" を行い、// タスク2 は値を "set" する。
      let t1 = tokio::spawn(async move {
          let _res = client.get("hello").await;
      });

      let t2 = tokio::spawn(async move {
          let _res = client.set("foo", "bar".into()).await;
      });

      t1.await?;
      t2.await?;
      Ok(())
  }
  ```

- このコードは、両方のタスクが `client` にアクセスする必要があるため、このままではコンパイルを通せない

## メッセージ受け渡し

- この問題の解決策として、「メッセージ受け渡し」パターンを用いる
  - `client` リソースを管理する専用のタスクをスポーンする
  - `client` と相互作用したい場合は、このタスクとメッセージをやり取りする
  - このパターンを使うことで、コネクションを一つに絞りつつ、複数のタスクを並行して実行出来る
  - また、チャンネルはバッファとしても機能する

## Tokio のチャンネルプリミティブ

- Tokio は以下にあげる複数の異なる機能を持った多くのチャンネルを提供する：
  - `mpsc`: multi-producer, single-consumer 型のチャンネル（たくさんの値を送ることができる）
  - `oneshot`: single-producer, single-consumer 型のチャンネル（一つの値を送ることができる）
  - `broadcast`: multi-producer, multi-consumer 型のチャンネル（受信側はすべての値を見ることができる）
  - `watch`: single-producer, multi-consumer 型のチャンネル（たくさんの値を送ることができるが、履歴は残らない. 受信側は最新の値のみを見ることができる）
- また、multi-producer, multi-consumer 型のチャネルで、それぞれのメッセージを見ることができる消費者（受信者）は1つだけ、というようなチャンネルがほしい場合は、`async-channel` クレートを使える

## メッセージ型の定義

- メッセージのやり取りを取り扱うにあたって、まず、メッセージの種類を定義する

  ```rust
  #[derive(Debug)]
  enum Command {
      Get {
          key: String
      },
      Set {
          key: String,
          val: Bytes
      }
  }
  ```

## チャンネルを作成

- main 関数内で、 `tokio::sync::mpsc` を用いてチャンネルを作成する
  - tokio の mpsc チャンネルはバッファを持つので、そのサイズも指定する
    - もしメッセージが受信されるよりも早く次のメッセージが送信されたら、チャンネルはそれを蓄える
    - メッセージが指定の個数だけ蓄えられると、受信側がメッセージを除去するまでの間、`send(...).await` の呼び出しはスリープする

  ```rust
  // --snip--

  #[tokio::main]
  async fn main() -> Result<()>{
      // 最大で 32 のキャパシティを持つチャンネルを作成
      let (tx, rx) = mpsc::channel::<Command>(32);

      Ok(())
  }
  ```

- まず、送信側の取り扱い方を紹介する
  - `mpsc::channel` で作成した `Sender` を `clone` し、各タスクに渡せばよい
  - 渡した先で `send(...送信内容...).await` すれば、データをレシーバ側に送ることができる

  ```rust
  #[tokio::main]
  async fn main() -> Result<()>{
      // 最大で 32 のキャパシティを持つチャンネルを作成
      let (tx, mut rx) = mpsc::channel::<Command>(32);

      // 2つのタスクを spawn する。
      // タスク1 はキーによる "get" を行い、
      // タスク2 は値を "set" する。
      let tx1 = tx.clone();
      tokio::spawn(async move {
          // レシーバに GET コマンドを送信する
          let _ = tx1.send(Command::Get { key: "hello".to_string() }).await;
      });
      
      let tx2 = tx;
      tokio::spawn(async move {
          // レシーバに SET コマンドを送信する
          let _ = tx2.send(Command::Set { key: "foo".to_string(), val: "bar".into() }).await;
      });
      Ok(())
  }
  ```

- なお、すべての `Sender` が drop された場合、レシーバの `recv` メソッドの返り値は `None` になる
  - `recv` の返り値が `None` であることはすべての送信者がいなくなって、チャンネルが閉じられたことを意味する

## "マネージャー" タスクをスポーンする

- 各送信機からのメッセージを処理するタスクをスポーンする
- このタスクでは、まず最初に Redis クライアントコネクションが確立され、そのあと、送信機から受信したコマンドを Redis に送信する

  ```rust
  // --snip--

  #[tokio::main]
  async fn main() -> Result<()> {
      // 最大で 32 のキャパシティを持つチャンネルを作成
      let (tx, mut rx) = mpsc::channel::<Command>(32);

      tokio::spawn(async move {
          // TCP コネクションを確立
          let mut client = client::connect("127.0.0.1:6379").await.unwrap();

          // rx.recv().await の結果が None のときは
          // すべての送信機がドロップしているので
          // コマンドを受け付ける必要がなくなる
          while let Some(cmd) = rx.recv().await {
              match cmd {
                  Command::Get { key } => {
                      let _ = client.get(&key).await;
                  }
                  Command::Set { key, val } => {
                      let _ = client.set(&key, val).await;
                  }
              }
          }
      });

      // 2つのタスクを spawn する。
      // タスク1 はキーによる "get" を行い、
      // タスク2 は値を "set" する。
      let tx1 = tx.clone();
      let t1 = tokio::spawn(async move {
          // レシーバに GET コマンドを送信する
          let _ = tx1
              .send(Command::Get {
                  key: "hello".to_string(),
              })
              .await;
      });

      let tx2 = tx;
      let t2 = tokio::spawn(async move {
          // レシーバに SET コマンドを送信する
          let _ = tx2
              .send(Command::Set {
                  key: "foo".to_string(),
                  val: "bar".into(),
              })
              .await;
      });

      // プロセスが終了してしまう前に
      // コマンドが完了したことを保証するため
      // main 関数の一番下で join ハンドルを .await
      t1.await?;
      t2.await?;
      manager.await?;
      Ok(())
  }
  ```

## レスポンスを受け取る

- 最後に、"マネージャー"タスク内で受け取った Redis サーバからのレスポンスメッセージを各タスクに共有する
- そのために `oneshot` チャンネルを用いる

- `oneshot` も `mpsc` 同じく `Sender` と `Receiver` を対生成して使用する
  - ただしキャパシティの指定はない
  - また、ハンドルはどちらもクローン出来ない
  - `mpsc` とは異なり、データの受信側から送信側に送信機を送りつけて使用する

- 今回は、各タスクから、"マネージャー"タスクに `Sender` を渡し、`Receiver` は "マネージャー"タスクからの返事を受け取るために用いる

- まず、各タスクから"マネージャー"タスクに送る 'Command' 構造体に、送信機 `Responder` を追加する

  ```rust
  // --snip--
  use tokio::sync::{mpsc, oneshot};

  // resp の型は、"マネージャー" タスクの
  // client.get(...).await, client.set(...).await
  // の返り値の型を基準に決定
  #[derive(Debug)]
  enum Command {
      Get {
          key: String,
          resp: Responder<Option<Bytes>>,
      },
      Set {
          key: String,
          val: Bytes,
          resp: Responder<()>,
      },
  }

  type Responder<T> = oneshot::Sender<Result<T>>;

  // --snip--
  ```

- 次に送信機と受信機を対生成して、"マネージャー" タスクに送信機を送りつける
- さらに、受信機でレスポンスが返ってくるのを待って内容を表示する

  ```rust
      // --snip--

      // 2つのタスクを spawn する。
      // タスク1 はキーによる "get" を行い、
      // タスク2 は値を "set" する。
      let tx1 = tx.clone();
      let t1 = tokio::spawn(async move {
          // "マネージャー"タスクから Redis サーバとの通信結果を受け取るためのチャンネルを作成
          let (resp_tx, resp_rx) = oneshot::channel();

          // マネージャータスク内のレシーバに GET コマンドを送信する
          // 送信が失敗したときにプログラムが終了するように unwrap する
          // コマンドと一緒に、マネージャータスクが通信結果を送り返すための送信機も添える
          let _ = tx1
              .send(Command::Get {
                  key: "hello".to_string(),
                  resp: resp_tx,
              })
              .await
              .unwrap();

          // マネージャータスクから、Redis の通信結果が返ってくるのを待つ
          let response = resp_rx.await;
          println!("GOT: {:?}", response)
      });

      let tx2 = tx;
      let t2 = tokio::spawn(async move {
          // "マネージャー"タスクから Redis サーバとの通信結果を受け取るためのチャンネルを作成
          let (resp_tx, resp_rx) = oneshot::channel();

          // レシーバに SET コマンドを送信する
          // 送信が失敗したときにプログラムが終了するように unwrap する
          // コマンドと一緒に、マネージャータスクが通信結果を送り返すための送信機も添える
          let _ = tx2
              .send(Command::Set {
                  key: "foo".to_string(),
                  val: "bar".into(),
                  resp: resp_tx,
              })
              .await
              .unwrap();

          // マネージャータスクから、Redis の通信結果が返ってくるのを待つ
          let response = resp_rx.await;
          println!("GOT: {:?}", response);
      });

      // --snip--
  ```

- 最後に "マネージャー" タスクで、各タスクから受け取った送信機で Redis サーバとの通信の結果を、各タスクに送り返す処理を追加する

```rust
    let manager = tokio::spawn(async move {
        // TCP コネクションを確立
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        // rx.recv().await の結果が None のときは
        // すべての送信機がドロップしているので
        // コマンドを受け付ける必要がなくなる
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key, resp } => {
                    let res: Result<Option<Bytes>> = client.get(&key).await;

                    // Redis サーバとの通信結果を返却
                    //     なお、send メソッドがエラーをはいた場合は無視する
                    //     それは受信側（元のタスク）の受信機がすでにドロップされており
                    //     受信側にレスポンスへの興味がすでにないことを表すから
                    let _ = resp.send(res);
                }
                Command::Set { key, val, resp } => {
                    let res: Result<()> = client.set(&key, val).await;

                    // Redis サーバとの通信結果を返却
                    //     なお、send メソッドがエラーをはいた場合は無視する
                    //     それは受信側（元のタスク）の受信機がすでにドロップされており
                    //     受信側にレスポンスへの興味がすでにないことを表すから
                    let _ = resp.send(res);
                }
            }
        }
    });
```
