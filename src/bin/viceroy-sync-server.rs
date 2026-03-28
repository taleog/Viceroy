fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Viceroy sync server");
        println!();
        println!("Usage:");
        println!("  viceroy-sync-server");
        println!("  viceroy-sync-server --help");
        println!();
        println!("Environment:");
        println!("  VICEROY_SYNC_SERVER_BIND        Bind address, default 0.0.0.0:8787");
        println!("  VICEROY_SYNC_SERVER_DATABASE    SQLite path, default ./viceroy-sync-server.db");
        println!("  VICEROY_SYNC_SERVER_AUTH_TOKEN  Optional bearer token");
        return;
    }

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let runtime = tokio::runtime::Runtime::new().expect("failed to create sync server runtime");
    if let Err(err) = runtime.block_on(viceroy::sync_server::run()) {
        eprintln!("Viceroy sync server failed: {err:#}");
        std::process::exit(1);
    }
}
