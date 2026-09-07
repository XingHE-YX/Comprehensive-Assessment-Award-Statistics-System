#[tokio::main]
async fn main() {
    if let Err(error) = zongce_web::run().await {
        tracing::error!(error_kind = error.kind(), "应用启动失败");
        eprintln!("应用启动失败：{error}");
        std::process::exit(1);
    }
}
