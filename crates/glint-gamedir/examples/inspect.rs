//! Prints what the instance page would show for a game folder.
//! `cargo run -p glint-gamedir --example inspect -- <game dir> [server address]`

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().ok_or("usage: inspect <game dir> [server]")?);
    for w in glint_gamedir::worlds::list(&dir)? {
        println!(
            "world  {:?} ({}) {} {:?} seed={:?} {} bytes",
            w.name, w.folder, w.game_mode, w.version, w.seed, w.size
        );
    }
    for s in glint_gamedir::servers::list(&dir)? {
        println!("server {:?} {}", s.name, s.address);
    }
    for s in glint_gamedir::shots::list(&dir)? {
        println!("shot   {} {} bytes", s.name, s.size);
    }
    for f in glint_gamedir::logs::list(&dir)? {
        println!("log    {} {:?} {} bytes", f.id, f.kind, f.size);
    }
    if let Some(address) = args.next() {
        let status = glint_gamedir::servers::ping(&address).await?;
        println!(
            "ping   {} {}/{} {} ms, motd: {}",
            status.version,
            status.online,
            status.max,
            status.latency_ms,
            status
                .motd
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
        );
    }
    Ok(())
}
