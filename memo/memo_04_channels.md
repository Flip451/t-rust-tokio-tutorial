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
      
      let tx2 = tx.clone();
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

      let tx2 = tx.clone();
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
