use std::sync::Mutex;

struct CanIncrement {
    mutex: Mutex<i32>,
}

impl CanIncrement {
    fn increment(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock += 1;
    }
}

async fn increment_and_do_stuff(can_incr: &CanIncrement) {
    can_incr.increment();
    do_something_async().await;
}

async fn do_something_async() {}

#[tokio::main]
async fn main() {
    let m = Mutex::new(1);
    let c = CanIncrement { mutex: m };
    tokio::spawn(async move { increment_and_do_stuff(&c).await });
}
