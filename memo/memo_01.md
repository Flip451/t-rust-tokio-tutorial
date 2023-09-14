# イントロダクション

## 目次

- [イントロダクション](#イントロダクション)
  - [目次](#目次)
  - [準備](#準備)
  - [Hello Tokio](#hello-tokio)
  - [非同期プログラミングとは？](#非同期プログラミングとは)
  - [`async`/`await`](#asyncawait)
  - [Async な main 関数](#async-な-main-関数)

## 準備

- Mini-Redis サーバーをインストール
  - `$ cargo install mini-redis`

- Mini-Redis サーバーを起動してクエリを送信

  - ```sh
    $ mini-redis-server
    $ mini-redis-cli get foo
    (nil)
    ```

## Hello Tokio

- Mini-Redis サーバーに接続して `hello` というキーに `world` という値をセットし、セットした値を読むアプリを作成する
  - 場所：`projects/my-redis`

- プロジェクトの作成
  - `$ cargo new my-redis`

- 依存関係の追加
  - `$ cargo add tokio --features full`
  - `$ cargo add mini-redis`

- コードの記述

  **`main.rs`**

  ```rs
  use mini_redis::{client, Result};

  #[tokio::main]
  pub async fn main() -> Result<()> {
      // 指定したアドレスとの間の TCP コネクションを確立する
      // この処理はネットワークを介するので時間のかかる処理
      // この時間のかかる処理が mini-redis クレートでは非同期処理として実装されている
      // `await` が使われていることからこのことがわかる
      let mut client = client::connect("127.0.0.1:6379").await?;
      
      // "hello" というキーに "world" という値をセット
      client.set("hello", "world".into()).await?;

      // "hello" の値を取得
      let result = client.get("hello").await?;

      println!("got value from the server; result={:?}", result);

      Ok(())
  }
  ```

- 動作確認
  - Mini-Redis の起動
    - `$ mini-redis-server`
  - バイナリの動作確認

    - ```sh
      $ cargo run
      got value from the server; result=Some(b"world")
      ```

## 非同期プログラミングとは？

- 同期的なプログラムでは、プログラムは書いた順番通りに実行される
  - １行目 &rarr; ２行目 &rarr; ３行目 ...
  - そのため、処理に時間のかかる処理に差し掛かると、その処理が終わるまでスレッドはブロックされる

- 一方で、非同期なプログラムでは、すぐに終わらない処理に遭遇したら、そのタスクはバックグラウンドで一旦棚上げされる
  - その間、スレッドはブロックされずに他の処理を続けることができる
  - バックグラウンドで棚上げされていた時間のかかる処理が完了したら、タスクは中断を解除されて中断していたところから再開される

## `async`/`await`

- Rust では非同期プログラミングは、`async`/`await` という機能で実装されている
  - 非同期処理を含む関数には `async` キーワードが付けられる
    - `async` 関数は非同期で実行される
    - Rust は**コンパイル時に** `async fn` を非同期処理を行うルーチンに変換する

  - `async fn` の内部で `.await` するとスレッドに制御が戻る

  - Rust では、非同期関数は lazy
    - ただ呼び出すだけだと、「処理を表す値」が返ってくる（クロージャが返ってくるようなイメージ）
    - 実際に処理を行うには、この「処理を表す値」に対して `.await` 演算子を使う必要がある

- `async`/`await` の使用例：

  ```rust
  async fn say_world() {
      println!("world");
  }

  #[tokio::main]
  async fn main() {
      // ただ呼び出すだけだと中身は実行されない
      let op = say_world();

      // さきに以下の文が実行される
      println!("hello");

      // .await を使って初めて中身が実行される
      op.await;
  }
  ```

- 上記のコードの実行結果：

  ```sh
  hello
  world
  ```

## Async な main 関数

- 非同期プログラミングを行う場合、非同期関数は必ずランタイムによって実行されなければならない
  - ランタイム：非同期タスクのスケジューラ・イベント駆動I/O・タイマーなどを提供する
  - ランタイムの例：Tokio
  - このランタイムを利用するには main 関数でラインタイムをスタートさせる必要がある

- ランタイムとして tokio を用いる場合、`main` 関数に `#[tokio::main]` というマクロをつけて、`async fn main` とする必要がある
  - こうすると、マクロによって、`async fn main` が同期的な `fn main` に変換される
  - この変換ののちに `fn main` 内でランタイムの初期化処理や非同期処理の実行が行われる

- 例：以下のコードはマクロが展開されると、さらにその下に示すよなコードになる：

  ```rust
  #[tokio::main]
  async fn main() {
      println!("hello");
  }
  ```

  ```rust
  fn main() {
      let mut rt = tokio::runtime::Runtime::new().unwrap();
      rt.block_on(async {
        println!("helo");
      })
  }```

## cargo の `features` について

- このチュートリアルでは、Tokio に依存するときは、`full` という `feature` フラグを有効化する

```toml
tokio = { version = "1", features = ["full"] }
```

- Tokio は、多くの機能（TCP, UDP, Unix ソケット, タイマー, sync に関するユーティリティ、複数種類のスケジューラなど）を提供する
- ほとんどのアプリはこれらすべての機能を用いるわけではないので、コンパイル時間の削減に当たって、この `features` フラグの内容を変更するべし
