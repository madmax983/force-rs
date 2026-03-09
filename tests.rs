use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let lock = Arc::new(RwLock::new(0));
    let mut tasks = vec![];
    for _ in 0..10 {
        let lock = lock.clone();
        tasks.push(tokio::spawn(async move {
            let _read = lock.read().await;
            let _write = lock.write().await;
            println!("done");
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
}
