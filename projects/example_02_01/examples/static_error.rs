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