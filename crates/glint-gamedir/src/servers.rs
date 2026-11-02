//! The multiplayer server list (`servers.dat`) and Server List Ping.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use fastnbt::Value;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{Error, Result, io, nbt};

/// Whether the game downloads a server's resource pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resources {
    Prompt,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    pub address: String,
    /// Base64 PNG, as the game stores it.
    #[serde(default)]
    pub icon: Option<String>,
    pub resources: Resources,
}

fn path(game_dir: &Path) -> std::path::PathBuf {
    game_dir.join("servers.dat")
}

pub fn list(game_dir: &Path) -> Result<Vec<Server>> {
    let file = path(game_dir);
    if !file.is_file() {
        return Ok(Vec::new());
    }
    let root = nbt::read(&file)?;
    let Some(Value::List(entries)) = nbt::get(&root, &["servers"]) else {
        return Ok(Vec::new());
    };
    Ok(entries
        .iter()
        .filter(|e| nbt::as_i64(nbt::get(e, &["hidden"])) != Some(1))
        .map(|e| Server {
            name: nbt::as_str(nbt::get(e, &["name"]))
                .unwrap_or("Minecraft Server")
                .to_owned(),
            address: nbt::as_str(nbt::get(e, &["ip"]))
                .unwrap_or_default()
                .to_owned(),
            icon: nbt::as_str(nbt::get(e, &["icon"])).map(str::to_owned),
            resources: match nbt::as_i64(nbt::get(e, &["acceptTextures"])) {
                Some(1) => Resources::Enabled,
                Some(0) => Resources::Disabled,
                _ => Resources::Prompt,
            },
        })
        .collect())
}

/// Replaces the whole list, in order.
pub fn save(game_dir: &Path, servers: &[Server]) -> Result<()> {
    std::fs::create_dir_all(game_dir).map_err(|e| io(game_dir, e))?;
    let entries = servers
        .iter()
        .map(|s| {
            let mut map = HashMap::from([
                ("name".to_owned(), Value::String(s.name.clone())),
                ("ip".to_owned(), Value::String(s.address.trim().to_owned())),
            ]);
            if let Some(icon) = &s.icon {
                map.insert("icon".into(), Value::String(icon.clone()));
            }
            match s.resources {
                Resources::Enabled => map.insert("acceptTextures".into(), Value::Byte(1)),
                Resources::Disabled => map.insert("acceptTextures".into(), Value::Byte(0)),
                Resources::Prompt => None,
            };
            Value::Compound(map)
        })
        .collect();
    let root = Value::Compound(HashMap::from([(
        "servers".to_owned(),
        Value::List(entries),
    )]));
    nbt::write(&path(game_dir), &root, false)
}

/// One styled run of a server's message of the day.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MotdSpan {
    pub text: String,
    /// CSS colour, when the server set one.
    pub color: Option<String>,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: String,
    pub protocol: i64,
    pub online: i64,
    pub max: i64,
    /// Some player names, when the server shares them.
    pub sample: Vec<String>,
    pub motd: Vec<MotdSpan>,
    /// `data:image/png;base64,…`
    pub favicon: Option<String>,
    pub latency_ms: u64,
}

const TIMEOUT: Duration = Duration::from_secs(5);

/// Asks a server for its status the way the game's server list does.
pub async fn ping(address: &str) -> Result<Status> {
    tokio::time::timeout(TIMEOUT * 2, ping_inner(address))
        .await
        .map_err(|_| Error::Ping("timed out".into()))?
}

fn split_address(address: &str) -> (String, u16) {
    let address = address.trim();
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            (host.to_owned(), port.parse().unwrap_or(25565))
        }
        _ => (address.to_owned(), 25565),
    }
}

/// Servers without an explicit port may publish their real host in a
/// `_minecraft._tcp` SRV record, like the game resolves them.
async fn resolve_srv(host: &str) -> Option<(String, u16)> {
    let resolver = hickory_resolver::TokioResolver::builder_tokio()
        .ok()?
        .build();
    let lookup = tokio::time::timeout(
        Duration::from_secs(3),
        resolver.srv_lookup(format!("_minecraft._tcp.{host}.")),
    )
    .await
    .ok()?
    .ok()?;
    let record = lookup.iter().min_by_key(|r| r.priority())?;
    Some((
        record.target().to_utf8().trim_end_matches('.').to_owned(),
        record.port(),
    ))
}

async fn ping_inner(address: &str) -> Result<Status> {
    let (mut host, mut port) = split_address(address);
    if !address.contains(':')
        && host.parse::<std::net::IpAddr>().is_err()
        && let Some((target, target_port)) = resolve_srv(&host).await
    {
        (host, port) = (target, target_port);
    }
    let perr = |e: std::io::Error| Error::Ping(e.to_string());
    let mut stream = tokio::time::timeout(TIMEOUT, TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| Error::Ping("could not connect in time".into()))?
        .map_err(perr)?;
    stream.set_nodelay(true).ok();

    let mut handshake = vec![0x00];
    write_varint(&mut handshake, 767);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);
    send_packet(&mut stream, &handshake).await.map_err(perr)?;
    send_packet(&mut stream, &[0x00]).await.map_err(perr)?;

    let _length = read_varint(&mut stream).await?;
    let _id = read_varint(&mut stream).await?;
    let json_len = read_varint(&mut stream).await? as usize;
    if json_len > 1 << 21 {
        return Err(Error::Ping("status too large".into()));
    }
    let mut json = vec![0; json_len];
    stream.read_exact(&mut json).await.map_err(perr)?;

    let started = Instant::now();
    let mut ping = vec![0x01];
    ping.extend_from_slice(&42i64.to_be_bytes());
    send_packet(&mut stream, &ping).await.map_err(perr)?;
    // Some servers close instead of answering the ping; the status still counts.
    let latency_ms = match tokio::time::timeout(TIMEOUT, async {
        read_varint(&mut stream).await?;
        let mut pong = [0u8; 9];
        stream.read_exact(&mut pong).await.map_err(perr)
    })
    .await
    {
        Ok(Ok(_)) => started.elapsed().as_millis() as u64,
        _ => 0,
    };

    let value: serde_json::Value =
        serde_json::from_slice(&json).map_err(|e| Error::Ping(e.to_string()))?;
    let players = &value["players"];
    Ok(Status {
        version: value["version"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        protocol: value["version"]["protocol"].as_i64().unwrap_or_default(),
        online: players["online"].as_i64().unwrap_or_default(),
        max: players["max"].as_i64().unwrap_or_default(),
        sample: players["sample"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| p["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        motd: motd(&value["description"]),
        favicon: value["favicon"].as_str().map(str::to_owned),
        latency_ms,
    })
}

async fn send_packet(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    let mut packet = Vec::with_capacity(body.len() + 5);
    write_varint(&mut packet, body.len() as i32);
    packet.extend_from_slice(body);
    stream.write_all(&packet).await
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7f == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32> {
    let mut value = 0u32;
    for i in 0..5 {
        let byte = stream
            .read_u8()
            .await
            .map_err(|e| Error::Ping(e.to_string()))?;
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(Error::Ping("malformed packet".into()))
}

const NAMED: [(&str, char, &str); 16] = [
    ("black", '0', "#000000"),
    ("dark_blue", '1', "#0000aa"),
    ("dark_green", '2', "#00aa00"),
    ("dark_aqua", '3', "#00aaaa"),
    ("dark_red", '4', "#aa0000"),
    ("dark_purple", '5', "#aa00aa"),
    ("gold", '6', "#ffaa00"),
    ("gray", '7', "#aaaaaa"),
    ("dark_gray", '8', "#555555"),
    ("blue", '9', "#5555ff"),
    ("green", 'a', "#55ff55"),
    ("aqua", 'b', "#55ffff"),
    ("red", 'c', "#ff5555"),
    ("light_purple", 'd', "#ff55ff"),
    ("yellow", 'e', "#ffff55"),
    ("white", 'f', "#ffffff"),
];

#[derive(Clone, Default)]
struct Style {
    color: Option<String>,
    bold: bool,
    italic: bool,
}

/// Flattens a chat component (or a plain string with `§` codes) into styled runs.
pub fn motd(description: &serde_json::Value) -> Vec<MotdSpan> {
    let mut spans = Vec::new();
    walk(description, &Style::default(), &mut spans);
    spans.retain(|s| !s.text.is_empty());
    spans
}

fn walk(value: &serde_json::Value, parent: &Style, out: &mut Vec<MotdSpan>) {
    match value {
        serde_json::Value::String(text) => legacy(text, parent, out),
        serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, parent, out)),
        serde_json::Value::Object(map) => {
            let mut style = parent.clone();
            if let Some(color) = map.get("color").and_then(|c| c.as_str()) {
                style.color = if color.starts_with('#') {
                    Some(color.to_owned())
                } else {
                    NAMED
                        .iter()
                        .find(|(n, _, _)| *n == color)
                        .map(|(_, _, hex)| (*hex).to_owned())
                };
            }
            if let Some(b) = map.get("bold").and_then(|b| b.as_bool()) {
                style.bold = b;
            }
            if let Some(i) = map.get("italic").and_then(|i| i.as_bool()) {
                style.italic = i;
            }
            if let Some(text) = map.get("text") {
                walk(text, &style, out);
            }
            if let Some(extra) = map.get("extra") {
                walk(extra, &style, out);
            }
        }
        _ => {}
    }
}

fn legacy(text: &str, base: &Style, out: &mut Vec<MotdSpan>) {
    let mut style = base.clone();
    let mut current = String::new();
    let mut chars = text.chars();
    let flush = |current: &mut String, style: &Style, out: &mut Vec<MotdSpan>| {
        if !current.is_empty() {
            out.push(MotdSpan {
                text: std::mem::take(current),
                color: style.color.clone(),
                bold: style.bold,
                italic: style.italic,
            });
        }
    };
    while let Some(c) = chars.next() {
        if c != '§' {
            current.push(c);
            continue;
        }
        let Some(code) = chars.next().map(|c| c.to_ascii_lowercase()) else {
            break;
        };
        flush(&mut current, &style, out);
        match code {
            'l' => style.bold = true,
            'o' => style.italic = true,
            'r' => style = base.clone(),
            c => {
                if let Some((_, _, hex)) = NAMED.iter().find(|(_, code, _)| *code == c) {
                    style = Style {
                        color: Some((*hex).to_owned()),
                        ..Style::default()
                    };
                }
            }
        }
    }
    flush(&mut current, &style, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn server_list_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list(tmp.path()).unwrap().is_empty());
        let servers = vec![
            Server {
                name: "Home".into(),
                address: "localhost".into(),
                icon: None,
                resources: Resources::Prompt,
            },
            Server {
                name: "Big".into(),
                address: "play.example.net:25570".into(),
                icon: Some("iVBOR".into()),
                resources: Resources::Enabled,
            },
        ];
        save(tmp.path(), &servers).unwrap();
        assert_eq!(list(tmp.path()).unwrap(), servers);
    }

    #[test]
    fn motd_formats() {
        let plain = motd(&serde_json::json!("§aGreen §lBold§r plain"));
        assert_eq!(
            plain[0],
            MotdSpan {
                text: "Green ".into(),
                color: Some("#55ff55".into()),
                bold: false,
                italic: false
            }
        );
        assert!(plain[1].bold && plain[1].text == "Bold");
        assert_eq!(plain[2].color, None);

        let component = motd(
            &serde_json::json!({"text": "", "extra": [{"text": "Hi", "color": "gold"}, {"text": "!", "color": "#123456", "bold": true}]}),
        );
        assert_eq!(component.len(), 2);
        assert_eq!(component[0].color.as_deref(), Some("#ffaa00"));
        assert_eq!(component[1].color.as_deref(), Some("#123456"));
    }

    #[test]
    fn addresses_default_to_the_minecraft_port() {
        assert_eq!(split_address("example.net"), ("example.net".into(), 25565));
        assert_eq!(
            split_address(" example.net:25570 "),
            ("example.net".into(), 25570)
        );
    }

    /// A tiny fake server answering one status request and one ping.
    #[tokio::test]
    async fn pings_a_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            for _ in 0..2 {
                let len = read_varint(&mut socket).await.unwrap();
                let mut skip = vec![0; len as usize];
                socket.read_exact(&mut skip).await.unwrap();
            }
            let json = r#"{"version":{"name":"1.21.4","protocol":769},"players":{"max":20,"online":3,"sample":[{"name":"Steve","id":"x"}]},"description":{"text":"Hello"},"favicon":"data:image/png;base64,AA=="}"#;
            let mut body = vec![0x00];
            write_varint(&mut body, json.len() as i32);
            body.extend_from_slice(json.as_bytes());
            send_packet(&mut socket, &body).await.unwrap();
            let len = read_varint(&mut socket).await.unwrap();
            let mut ping = vec![0; len as usize];
            socket.read_exact(&mut ping).await.unwrap();
            send_packet(&mut socket, &ping).await.unwrap();
        });

        let status = ping(&format!("127.0.0.1:{port}")).await.unwrap();
        assert_eq!(
            (status.online, status.max, status.version.as_str()),
            (3, 20, "1.21.4")
        );
        assert_eq!(status.sample, ["Steve"]);
        assert_eq!(status.motd[0].text, "Hello");
        assert!(status.favicon.is_some());
    }
}
