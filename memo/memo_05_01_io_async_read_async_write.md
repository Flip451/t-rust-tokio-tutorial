# I/O

## 目次

- [I/O](#io)
  - [目次](#目次)
  - [概要](#概要)
  - [`AsyncRead` と `AsyncWrite`](#asyncread-と-asyncwrite)
    - [`async fn read()`](#async-fn-read)
    - [`async fn read_to_end()`](#async-fn-read_to_end)
    - [`async fn write()`](#async-fn-write)
    - [`async fn write_all`](#async-fn-write_all)
  - [ヘルパー関数](#ヘルパー関数)

## 概要

- tokio の I/O は `std` のものとほぼ同じ
- しかし非同期という点で異なる
- 読み込み用のトレイト（`AsyncRead`）と書き込み用のトレイト（`AsyncWrite`）がある
  - `TcpStream`, `File`, `Stdout` などがこれらのトレイトを必要に応じて実装している
  - また、`Vec<u8>` や `&[u8]` などの多くのデータ構図に対しても実装されている
    - これにより、 reader / writer が要求される個所でバイト配列を用いることができる

## `AsyncRead` と `AsyncWrite`

- これら二つのトレイトは、バイトストリーム（バイト単位のデジタルデータの『ひと続き』のこと）への読み書きを非同期に行う機能を実装している
- これらのトレイトのメソッドを直接呼び出すことはない
- 代わりに `AsyncReadExt` と `AsyncWriteExt` が提供するユーティリティメソッドを用いることが多い

### `async fn read()`

- `AsyncReadExt::read` は、データをバッファへと読み込んで、何バイト読み込まれたかを返す非同期メソッド
  - `read` が `Ok(0)` を返して来たら、そのストリームが閉じられたことを意味する
  - `Ok(0)` が返ってきたら、それ以上 `read()` しても、即座に `Ok(0)` が返ってくる
  - たとえば、`TcpStream` インスタンスで `read()` が `Ok(0)` を返して来たら、ソケットの読み込み側が閉じられたことを意味する

- 例：

  ```rust
  use anyhow::Result;
  use tokio::{fs::File, io::AsyncReadExt};

  #[tokio::main]
  async fn main() -> Result<()> {
      let mut f = File::open("foo.txt").await?;
      let mut buffer = [0; 10];

      // 最大 10 バイト読み込む
      let n = f.read(&mut buffer).await?;
      println!("The bytes: {:?}", &buffer[..n]);
      Ok(())
  }
  ```

- 例２：ループを回してファイルの内容をすべて読み込む

  ```rust
  use anyhow::Result;
  use tokio::{fs::File, io::AsyncReadExt};

  #[tokio::main]
  async fn main() -> Result<()>{
      let mut f = File::open("foo.txt").await?;
      let mut buffer = [0; 4];

      loop {
          match f.read(&mut buffer).await? {
              0 => break,
              n => {
                  println!("{}", String::from_utf8(buffer[..n].to_vec()).unwrap())
              }
          }
      }

      Ok(())
  }
  ```

  - 実行結果の例：

    ```sh
    Hell
    o, w
    orld
    !
    Th
    is i
    s fr
    om a
    sync
     rus
    t.

    ```

### `async fn read_to_end()`

- `AsyncReadExt::read_to_end` は、EOF（End Of File） にいたるまでストリームからすべてのバイトを読み込む
- `read` と異なり、引数には `&Vec<u8>` を受け取るので注意（`read` では `&[u8]` を受け取る）

  ```rust
  use anyhow::{Result, Ok};
  use tokio::{fs::File, io::AsyncReadExt};

  #[tokio::main]
  async fn main() -> Result<()>{
      let mut f = File::create("foo.txt").await?;
      let mut buffer = vec![];
      
      let n = f.read_to_end(&mut buffer).await?;
      println!("The bytes: {:?}, size: {}", buffer, n);
      Ok(())
  }
  ```

### `async fn write()`

- `AsyncWriteExt::write` はバッファを writer に書き込み、何バイト書き込んだかを返す非同期メソッド
- ファイルを開くときに書き込みモードで開く必要があることに注意

  ```rust
  use anyhow::Result;
  use tokio::{fs::File, io::AsyncWriteExt};

  #[tokio::main]
  async fn main() -> Result<()> {
      // `File::open` ではなく `File::create` を使うと、
      // 書き込み専用モードでファイルを開ける
      let mut f = File::create("foo.txt").await?;

      let bytes = b"Hello, world!
  This is from async rust.
  ";

      // バイト列の先頭からいくつかを書き込む
      // 必ずしもすべてを書き込むわけではない
      let n = f.write(bytes).await?;

      println!(
          "Wrote the first {:?} bytes of {:?}",
          n,
          String::from_utf8(bytes.to_vec())
      );
      Ok(())
  }
  ```

### `async fn write_all`

- `AsyncWriteExt::write_all` は writer にバッファ全体を書き込む

  ```rust
  use anyhow::Result;
  use tokio::{fs::File, io::AsyncWriteExt};

  #[tokio::main]
  async fn main() -> Result<()>{
      let mut f = File::create("foo.txt").await?;
      
      f.write_all(b"Hello, world! This is result of AsyncWriteExt::write_all method.").await?;
      
      Ok(())
  }
  ```

## ヘルパー関数

- `std` と同様に、`tokio::io` モジュールは 標準入力、標準出力、標準エラー出力 を利用するための API や、数々の便利な関数を提供している
- 例えば、`tokio::io::copy` を用いると、reader から writer へとすべてのデータを非同期にコピーすることができる

  ```rust
  use anyhow::Result;
  use tokio::{fs::File, io};

  #[tokio::main]
  async fn main() -> Result<()>{
      // &[u8] は AsyncReader トレイトを実装している
      let mut reader: &[u8] = b"hello";
      let mut f = File::create("foo.txt").await?;

      // ファイルに `reader` の内容を書き写すことができる
      // pub async fn copy<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> io::Result<u64>
      // where
      // R: AsyncRead + Unpin + ?Sized,
      // W: AsyncWrite + Unpin + ?Sized,
      io::copy(&mut reader, &mut f).await?;

      Ok(())
  }
  ```

