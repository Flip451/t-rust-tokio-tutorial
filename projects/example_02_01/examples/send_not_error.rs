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
