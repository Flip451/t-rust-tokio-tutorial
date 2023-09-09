# spawning

## 目次

- [spawning](#spawning)
  - [目次](#目次)
  - [概要](#概要)
  - [準備](#準備)
  - [ソケットを受け付ける](#ソケットを受け付ける)
    - [並行性](#並行性)
    - [タスク](#タスク)
    - [`'static` 境界](#static-境界)
    - [`Send` 境界](#send-境界)
  - [値を保存する](#値を保存する)

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

### タスク

- Tokio における「タスク」は非同期のグリーンスレッドで `async` ブロックを `tokio::spawn` に渡すことで作成される

> - 「グリーンスレッド」：コンピュータプログラミングにおいて、オペレーティングシステムではなく、ランタイムライブラリや仮想マシン (VM) によってスケジュールされるスレッドのこと（Wikipedia より）

- `tokio::spawn` 関数は `JoinHandle` を返す
  - `spawn` 関数の呼び出し元は、この `JoinHandle` 使ってタスクとやり取りできる

- また、非同期ブロックは戻り値を持つことができる
  - 呼び出し元は、`JoinHandle` に `.await` を作用させることでこの戻り値（`Result` 型）を取得できる
    - この戻り値が `Err` になるのは、該当のタスクが実行中にエラーに遭遇した場合（これはタスクがパニックを起こしたか、ランタイムがシャットダウンしてタスクがキャンセルされたとき）

- 例：
  
  ```rust
  use anyhow::{Result, Ok};

  #[tokio::main]
  async fn main() -> Result<()>{
      // タスクを生成してハンドルを受け取る
      // ここでスポーンしたタスクはバックグラウンドで進められる
      let handle = tokio::spawn(async {
          // ここで何かしらの非同期処理を実行できる
          "return value"
      });

      // ここで何かしらの他の処理を進められる
      // ここで行われる処理は、上で生成したタスクと並行して進められる

      // 先のタスクの結果を受け付ける
      let out = handle.await?;
      println!("{}", out);
      Ok(())
  }
  ```

### `'static` 境界

- Tokio ランタイム上でタスクを `spawn` するときには、`spawn` に渡す `async` ブロックは `'static` 境界を満たさなければならない
  - つまり、スポーンされるタスクは、タスクの外部で所有されるデータへの参照を含んではならない

- 例えば、以下のコードはコンパイルエラーを起こす：

  ```rust
  use tokio::task;

  #[tokio::main]
  async fn main() {
      // v は main 関数に所有されている
      let v = vec![1, 2, 3];

      // async ブロック内でブロック外（main 関数）に所有権がある値を参照しようとしている
      task::spawn(async {
          println!("vec: {:?}", v);
      });
  }
  ```

  - このコードは以下のようなコンパイルエラーを起こす；

  ```sh
   --> examples/static_error.rs:7:17
    |
  7 |       task::spawn(async {
    |  _________________^
  8 | |         println!("vec: {:?}", v);
    | |                               - `v` is borrowed here
  9 | |     });
    | |_____^ may outlive borrowed value `v`
    |
    = note: async blocks are not executed immediately and must either take a reference or ownership of outside variables they use
  help: to force the async block to take ownership of `v` (and any other referenced variables), use the `move` keyword
    |
  7 |     task::spawn(async move {
    |                       ++++

  For more information about this error, try `rustc --explain E0373`.
  error: could not compile `example_02_01` (example "static_error") due to previous error
  ```

- このコードは、コンパイラが指示しているように、`async {...}` の代わりに `async move {...}` を使えば問題なく動くようになる
  - この場合、`async` ブロック内からブロック外のデータにアクセスしようとするとブロックが所有権を奪うので、タスクは必要なデータを自分で所持している &rarr; `'static` になる

- もし、データの一部が複数のタスクから非同期にアクセスされる場合、`Arc` などを用いてデータを複数タスク間で共有することになる（`Arc` については the book の 16 章参照）

- そもそも、なぜ `'static` である必要があるの？
  - コンパイラはスポーンされたタスクがどれだけ長生きするかわからないので、ライフタイムの計算のために、タスクは永遠に存在しうると仮定する
  - 「ある値が `'static` であること」は「その値をずっと保持すし続けたとしても不正な値になることはない」ということを表すので、永遠に存在しうるタスクは `'static` でなければならない  
    > ただし、ここでいう `'static` は、参照のライフタイムを表すためのものではなく、型に課される境界付けの `'static` であることに注意

### `Send` 境界

- Tokio ランタイム上でタスクを `spawn` するときには、`spawn` に渡す `async` ブロックは `Send` 境界を満たさなければならない
  - `tokio::spawn` で作成するタスクの `async` ブロック内で `.await` すると、その前後で処理に使われるスレッドが変わりうる（<https://qiita.com/legokichi/items/4f2c09330f90626600a6#tokiomainflavor--multi_thread-worker_threads--10> 参照）
  - なので、`async` ブロック内で `.await` を用いたときに、その前後に渡って存在している変数の中に `Send` を実装していないものがあってはならない
  - 逆に、`.await` の前後に渡って存在するすべてのデータが `Send` 境界を満たしていれば、`async` ブロック自体も `Send` 境界を満たす

- 例：以下のコードはコンパイルエラーを起こす：

  ```rust
  use std::rc::Rc;
  use tokio::task::yield_now;

  #[tokio::main]
  async fn main() {
      tokio::spawn(async {
          // Rc は Send を実装していない
          let rc = Rc::new("hello");

          // 制御を一旦スケジューラに返す
          // すると、この行の後の処理は、次にタスクが再開されたときに実行されることになる
          // --> .await の前後でタスクを処理するスレッドが変更されうる
          yield_now().await;

          println!("{}", rc);
      });
  }

  ```

  - コンパイルエラーの内容は以下の通り：

    ```sh
    error: future cannot be sent between threads safely
       --> examples/send_error.rs:6:18
        |
    6   |       tokio::spawn(async {
        |  __________________^
    7   | |         // Rc は Send を実装していない
    8   | |         let rc = Rc::new("hello");
    9   | |
    ...   |
    15  | |         println!("{}", rc);
    16  | |     });
        | |_____^ future created by async block is not `Send`
        |
        = help: within `[async block@examples/send_error.rs:6:18: 16:6]`, the trait `Send` is not implemented for `Rc<&str>`
    note: future is not `Send` as this value is used across an await
       --> examples/send_error.rs:13:21
        |
    8   |         let rc = Rc::new("hello");
        |             -- has type `Rc<&str>` which is not `Send`
    ...
    13  |         yield_now().await;
        |                     ^^^^^ await occurs here, with `rc` maybe used later
    ...
    16  |     });
        |     - `rc` is later dropped here
    note: required by a bound in `tokio::spawn`
       --> /home/flip451/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.32.0/src/task/spawn.rs:166:21
        |
    164 |     pub fn spawn<T>(future: T) -> JoinHandle<T::Output>
        |            ----- required by a bound in this function
    165 |     where
    166 |         T: Future + Send + 'static,
        |                     ^^^^ required by this bound in `spawn`

    error: could not compile `example_02_01` (example "send_error") due to previous error
    ```

- `async` ブロック内で `Send` でない値を用いても、それが `.await` をまたいで存在しなければ問題なく動作する
  - 例：

    ```rust
    use std::rc::Rc;
    use tokio::task::yield_now;

    #[tokio::main]
    async fn main() {
        tokio::spawn(async {
            // Rc は Send を実装していないが、{} で囲っているので
            // この {} の終わりにドロップされて `.await` の前後にまたがって存在はしない
            {
                let rc = Rc::new("hello");
                println!("{}", rc);
            }  // ここで rc はドロップする

            // この行の前後にまたがって非 Send のデータがあるとまずいが
            // 今回のケースは問題ない
            yield_now().await;

        });
    }
    ```

## 値を保存する

- 本題に戻って `process` 関数を実装する

- 今回の実装では、値を保存するために `HashMap` を用いる
  - `SET` コマンドは `HashMap` に値を挿入する
  - `GET` コマンドは `HashMap` から値を読みだす

- 以下に実装を示す：

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
          // それぞれのインバウンドコネクションに対して新しい「タスク」をスポーン
          // ソケットをその「タスク」に move して利用する
          tokio::spawn(async move {
              process(socket).await;
          });
      }
  }

  // リクエストを処理する非同期関数
  async fn process(socket: TcpStream) -> Result<()> {
      use mini_redis::Command::{self, Get, Set};
      use std::collections::HashMap;

      // この実装では、一旦各コネクションごとにひとつずつ db が作成される
      // つまり、あるコネクションの内部では db の変更が保持・反映されるが、
      // 他のコネクションから、この変更にアクセスすることはできない
      let mut db = HashMap::new();

      // mini-redis クレートで定義している `Connection` 構造体を用いることで、
      // バイト列ではなく Redis の「フレーム」を読み書き出来る
      let mut connection = Connection::new(socket);

      // 各コネクション内部で複数のコマンドを繰り返し受付できるように while ループを回す
      while let Some(frame) = connection.read_frame().await? {
          let response = match Command::from_frame(frame)? {
              Set(cmd) => {
                  // `Vec<u8> として保存する
                  db.insert(cmd.key().to_string(), cmd.value().to_vec());
                  Frame::Simple("OK".to_string())
              }
              Get(cmd) => {
                  if let Some(value) = db.get(cmd.key()) {
                      // `Frame::Bulk` はデータが Bytes` 型であることを期待する
                      // この型についてはのちほど解説する
                      Frame::Bulk(value.clone().into())
                  } else {
                      Frame::Null
                  }
              }
              cmd => panic!("unimplementd {:?}", cmd),
          };

          // クライアントへのレスポンスを書き込む
          connection.write_frame(&response).await?;
      }

      Ok(())
  }
  ```

  - この実装では、各コネクションごとに `db` を作成しているので、コネクション間で値を共有することができていないことに注意

- 動作確認：
  - `cargo run`
  - 別ターミナルで `cargo run --example hello-redis` すると `got value from the server; result=Some(b"world")` と表示される