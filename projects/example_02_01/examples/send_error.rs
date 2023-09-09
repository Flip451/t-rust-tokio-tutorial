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
