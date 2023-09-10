# 状態を共有する

## 目次

- [状態を共有する](#状態を共有する)
  - [目次](#目次)
  - [概要](#概要)
  - [戦略](#戦略)
  - [`bytes` クレートを依存に追加](#bytes-クレートを依存に追加)
  - [データベースの初期化処理](#データベースの初期化処理)
  - [タスクとスレッドと競合](#タスクとスレッドと競合)

## 概要

- 前節までの実装では、異なる TCP コネクション間で（異なるタスク間で）同じ db にアクセスすることができず、コネクション間で変更が共有されなかった
- この章では、この問題を解消するために `Arc` と `Mutex` を導入する

## 戦略

- Tokio でステートをタスク間で共有する方法はいくつかある
- 代表的なものは以下の二つ：
  1. `Mutex` を利用して共有されているステートを「ガード」する
     &rarr; シンプルなデータの共有に有効
  2. ステートを管理するための専用のタスクをスポーンして、メッセージの受け渡しによってステートを管理する
     &rarr; I/O プリミティブのような非同期処理を必要とするもので有効

- 今回の実装は前者ですすめる

## `bytes` クレートを依存に追加

```sh
cargo add bytes
```

- これにより、`Vec<u8>` を使う代わりに `bytes` クレートが提供する `Bytes` 型を使えるようになる
  - この型の方がネットワークプログラミングにおいては有用
  - この型の主な特徴は、`Vec<u8>` に shallow cloning を追加していること
    - つまり、`Bytes` に対して `clone()` を呼んでも内部データまで複製されず、参照のみが複製される

## データベースの初期化処理

- まず、db 用の型エイリアスを作成する

```rust
type Db = Arc<Mutex<HashMap<String, Bytes>>>;
```

- そして、各スレッドに、db のクローンを渡すように実装を書き換える

  ```rust
  use std::{collections::HashMap, sync::{Mutex, Arc}};

  use bytes::Bytes;
  use tokio::net::{TcpListener, TcpStream};

  use mini_redis::{Connection, Frame, Result};

  type Db = Arc<Mutex<HashMap<String, Bytes>>>;

  #[tokio::main]
  async fn main() -> Result<()> {
      // TCP 接続開始
      let listener = TcpListener::bind("127.0.0.1:6379").await?;
      let db = Db::new(Mutex::new(HashMap::new()));

      loop {
          // 接続を受け付け
          // 接続が実際に来るまでコードをブロック
          let (socket, address) = listener.accept().await?;
          let db = db.clone();

          // 接続元アドレスの表示
          println!("accept connection from {}", address);

          // リクエストの処理の実行
          // それぞれのインバウンドコネクションに対して新しい「タスク」をスポーン
          // ソケットをその「タスク」に move して利用する
          tokio::spawn(async move {
              let _ = process(socket, db).await;
          });
      }
  }

  // リクエストを処理する非同期関数
  async fn process(socket: TcpStream, db: Db) -> Result<()> {
      use mini_redis::Command::{self, Get, Set};

      // mini-redis クレートで定義している `Connection` 構造体を用いることで、
      // バイト列ではなく Redis の「フレーム」を読み書き出来る
      let mut connection = Connection::new(socket);

      // 各コネクション内部で複数のコマンドを繰り返し受付できるように while ループを回す
      while let Some(frame) = connection.read_frame().await? {
          let response = match Command::from_frame(frame)? {
              Set(cmd) => {
                  let mut db = db.lock().unwrap();
                  // `Vec<u8> として保存する
                  db.insert(cmd.key().to_string(), cmd.value().clone());
                  Frame::Simple("OK".to_string())
              }
              Get(cmd) => {
                  let db = db.lock().unwrap();
                  if let Some(value) = db.get(cmd.key()) {
                      // `Frame::Bulk` はデータが Bytes` 型であることを期待する
                      // この型についてはのちほど解説する
                      Frame::Bulk(value.clone())
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

- `Mutex` と `Arc` の使い方については the book の 16 章を参照すること

- **注意！！**： ここでは `Mutex` として `std::sync::Mutex` を用いる（`tokio::sync::Mutex` ではない）
  - 非同期の mutex は `.await` の呼び出しをまたいでロックされるような mutex のこと
  - 経験則として、非同期コード内で同期ミューテックスを使用することは、競合が少なく、`.await` の呼び出しにまたがってロックが保持されない限り、問題ない

## タスクとスレッドと競合

- 今回の実装では複数のタスクが同一のデータにアクセスするので、競合が起こる可能性がある
- 仮に、競合が激しい場合、他のタスクが共有データにアクセスしている間、タスクはミューテックスのロックが解除されるまでスレッドをブロックして待機しなければならない
  - これは、そのタスクを遅らせるのみならず、そのスレッドでの実行が予定されている他のタスクの開始も遅らせてしまう

> - なお、このような競合の問題はマルチスレッドスケジューラでのみ起こる. `current_thread` ランタイムを使用する場合はミューテックスが競合状態になることはない.

- このような競合に対しては以下のような解決策を図るのがよい（Tokio の mutex に切り替えることで解決することは滅多にないので注意）：
  - ステートを管理するための先任タスクを作成してメッセージの受け渡しを行う
  - ミューテックスをシャーディングする
    - シャーディングとは:
      - データベースの負荷分散方法の1種
      - 水平分割 とも呼ばれる
      - 同じテーブルを複数のデータベースに用意して、1つのテーブルに保存していたレコードを分散する事で各データベース内に保持されるレコードの量をへらす負荷分散の方法
  - ミューテックスを使わずに済むようにコードを再構築する

- 今回のケースでは、それぞれのキーが独立しているのでミューテックスをシャーディングするのがよい選択肢

- そこで、以下の変更を加える：
  - 競合への対策として、`ShardedDb` 型を以下のように定義する

    ```rust
    type ShardedDb = Arc<Vec<Mutex<HashMap<String, Bytes>>>>;
    ```

  - シャーディングされた db を作成するための関数も作成
  - どのキーをどの db に割り当てるかを決定する関数も作成

```diff
- use std::{collections::HashMap, sync::{Mutex, Arc}};
+ use std::{
+     collections::{hash_map::DefaultHasher, HashMap},
+     hash::{Hash, Hasher},
+     sync::{Arc, Mutex},
+ };

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};

use mini_redis::{Connection, Frame, Result};

- type Db = Arc<Mutex<HashMap<String, Bytes>>>;
+ type Db = Mutex<HashMap<String, Bytes>>;
+ type ShardedDb = Arc<Vec<Mutex<HashMap<String, Bytes>>>>;
+ 
+ // シャーディングされた db を作成する関数
+ fn new_sharded_db(num_shards: usize) -> ShardedDb {
+     let mut db = Vec::with_capacity(num_shards);
+     for _ in 0..num_shards {
+         let m = Mutex::new(HashMap::new());
+         db.push(m);
+     }
+     Arc::new(db)
+ }
+ 
+ // シャーディングされた db の中から該当の db を拾い上げる関数
+ fn get_db_from_sharded_db<'a>(shaded_db: &'a ShardedDb, key: &'a str) -> &'a Db {
+     let db = &shaded_db[hash(key) % shaded_db.len()];
+     db
+ }
+ 
+ // ハッシュ化関数
+ fn hash(key: &str) -> usize {
+     let mut s = DefaultHasher::new();
+     key.hash(&mut s);
+     s.finish() as usize
+ }

#[tokio::main]
async fn main() -> Result<()> {
    // TCP 接続開始
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

-   let db = Db::new(Mutex::new(HashMap::new()));
+   let db = new_sharded_db(5);

    loop {
        // 接続を受け付け
        // 接続が実際に来るまでコードをブロック
        let (socket, address) = listener.accept().await?;
        let db = db.clone();

        // 接続元アドレスの表示
        println!("accept connection from {}", address);

        // リクエストの処理の実行
        // それぞれのインバウンドコネクションに対して新しい「タスク」をスポーン
        // ソケットをその「タスク」に move して利用する
        tokio::spawn(async move {
            let _ = process(socket, db).await;
        });
    }
}

// リクエストを処理する非同期関数
- async fn process(socket: TcpStream, db: Db) -> Result<()> {
+ async fn process(socket: TcpStream, db: ShardedDb) -> Result<()> {
    use mini_redis::Command::{self, Get, Set};

    // mini-redis クレートで定義している `Connection` 構造体を用いることで、
    // バイト列ではなく Redis の「フレーム」を読み書き出来る
    let mut connection = Connection::new(socket);

    // 各コネクション内部で複数のコマンドを繰り返し受付できるように while ループを回す
    while let Some(frame) = connection.read_frame().await? {
        let response = match Command::from_frame(frame)? {
            Set(cmd) => {
+               let db = get_db_from_sharded_db(&db, cmd.key());
                let mut db = db.lock().unwrap();
                // `Vec<u8> として保存する
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
+               let db = get_db_from_sharded_db(&db, cmd.key());
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    // `Frame::Bulk` はデータが Bytes` 型であることを期待する
                    // この型についてはのちほど解説する
                    Frame::Bulk(value.clone())
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
