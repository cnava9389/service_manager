mod managers;
// #[tokio::main(worker_threads=2)]
// #[tokio::main(flavor = "current_thread")]
#[tokio::main]
async fn main() {

    match std::env::args().nth(1) {
        Some(x) => managers::service_manager::select_start(&x).await,
        None => managers::service_manager::start(),
    }
    println!("Server closed gracefully");
}
