# framing: Buf's "byte iterator" style APIs

## 目次

- [framing: Buf's "byte iterator" style APIs](#framing-bufs-byte-iterator-style-apis)
  - [目次](#目次)
  - [`parse_frame` 関数の実装（`Buf` トレイト内のカーソルを利用する）](#parse_frame-関数の実装buf-トレイト内のカーソルを利用する)

## `parse_frame` 関数の実装（`Buf` トレイト内のカーソルを利用する）

- 続いて、`parse_frame` 関数を実装する
  - この関数は、以下の二段階の処理として実装する
    1. フレームの終了インデックスを探して、ひとつのフレーム全体がバッファに含まれているかをチェックする
    2. フレームをパースして返却する

  ```rust
  use std::io::Cursor;

  use bytes::{BytesMut, Buf};
  use mini_redis::{Frame, Result};
  use mini_redis::frame::Error::Incomplete;
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

      // ストリームからフレームを一つ読み込む
      // EOF であれば None を返す
      pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
          //--snip--
      }

      // コネクションから単一のフレームをパースする関数
      // 1. 十分な量のデータがバッファされていたら、
      //    バッファからフレームを取り除いて、それを返却する
      // 2. フレームをパースするのに十分な量のデータがバッファされていなかったら
      //    Ok(None) を返却して、フレームを取り出せなかったが、
      //    追加でデータをバッファすれば問題ないはずであるか、
      //    読み込むべきデータがもうないかのいずれかであろうことを伝える
      // 3. 内部で問題が発生したら Err を返す
      fn parse_frame(&mut self) -> Result<Option<Frame>> {
          // Frame 構造体はパースの実行のためにカーソルを用いる
          let mut buf = Cursor::new(&self.buffer[..]);

          // まず、単一のフレームをパースするのに十分なデータがバッファに蓄えられているかをチェックする
          // その結果によって場合分けする：
          //      もし、十分な量蓄えられていたら、Ok(_) のアームに進む
          //      不十分なら Err(Incomplete) のアームに進む
          //      それ以外のエラーが発生しているようならエラーを返す
          match Frame::check(&mut buf) {
              Ok(_) => {
                  // parse に成功した場合、`Frame::check` 関数は
                  // カーソルの位置をフレームの終端にまで進める
                  // なので、カーソル位置を取得するとフレームの長さがわかる
                  let len = buf.position() as usize;

                  // パースの実行のためにカーソル位置を先頭に戻す
                  buf.set_position(0);

                  // フレームを取得する
                  // もし、エラーが返ってきたら、送られてきたフレームの内容が不正であることを表すので、
                  // このコネクションを切断する
                  let frame = Frame::parse(&mut buf)?;

                  // バッファされているデータのうち、パースし終えた部分を破棄する
                  //      advance(cnt) が読み込み用のバッファに対して呼ばれると、
                  //      `cnt` までのデータがすべて破棄される
                  self.buffer.advance(len);

                  // パースに成功したフレームを返す
                  Ok(Some(frame))
              },
              // 十分な量のデータがバッファされていなかった場合
              Err(Incomplete) => Ok(None),
              // エラーが発生した場合
              Err(e) => Err(e.into())
          }
      }

      // コネクションにフレームを書き込む
      pub async fn write_frame(&mut self) -> Result<()> {
          todo!()
      }
  }
  ```

- この実装では、`Buf` トレイトを実装する型として `std::io::Cursor` を用いている

- `Frame::check` 関数では、`Buf` トレイトの「バイトイテレータ」スタイルの API を利用している
  - これらの API （`get_u8`, `advance`）はデータを取得して、内部カーソルを移動させる

```rust
    /// Checks if an entire message can be decoded from `src`
    pub fn check(src: &mut Cursor<&[u8]>) -> Result<(), Error> {
        match get_u8(src)? {
            b'+' => {
                get_line(src)?;
                Ok(())
            }
            b'-' => {
                get_line(src)?;
                Ok(())
            }
            b':' => {
                let _ = get_decimal(src)?;
                Ok(())
            }
            b'$' => {
                if b'-' == peek_u8(src)? {
                    // Skip '-1\r\n'
                    skip(src, 4)
                } else {
                    // Read the bulk string
                    let len: usize = get_decimal(src)?.try_into()?;

                    // skip that number of bytes + 2 (\r\n).
                    skip(src, len + 2)
                }
            }
            b'*' => {
                let len = get_decimal(src)?;

                for _ in 0..len {
                    Frame::check(src)?;
                }

                Ok(())
            }
            actual => Err(format!("protocol error; invalid frame type byte `{}`", actual).into()),
        }
    }

// --snip--

fn get_u8(src: &mut Cursor<&[u8]>) -> Result<u8, Error> {
    if !src.has_remaining() {
        return Err(Error::Incomplete);
    }

    // get_u8 メソッド：
    // self の先頭から符号なし8ビット整数を取得する
    // さらに現在位置が1つ進む
    Ok(src.get_u8())
}

// --snip--
```

- Buf トレイトには他にも便利なメソッドが数多くある（詳細は、[API ドキュメント](https://docs.rs/bytes/1/bytes/buf/trait.Buf.html) を参照）
