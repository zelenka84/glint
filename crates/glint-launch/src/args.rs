use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glint_auth::LaunchIdentity;
use glint_meta::mojang::{Argument, ArgumentValue, VersionInfo};
use glint_meta::rules::rules_allow;
use glint_meta::{InstallPlan, MetaLayout, Os, Platform};

/// Per-launch settings that do not come from the version JSON.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub java: PathBuf,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    /// What [`crate::prepare`] returned; used for `${game_assets}` on old versions.
    pub game_assets: PathBuf,
    pub identity: LaunchIdentity,
    pub max_memory_mb: u32,
    pub extra_jvm_args: Vec<String>,
    pub resolution: Option<(u32, u32)>,
    /// Joins this server on start (1.20+ Quick Play), e.g. a Glint Link tunnel.
    pub quick_play_server: Option<String>,
    pub launcher_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

pub fn build_command(
    layout: &MetaLayout,
    version: &VersionInfo,
    plan: &InstallPlan,
    options: &LaunchOptions,
    platform: &Platform,
) -> LaunchCommand {
    let mut platform = platform.clone();
    if options.resolution.is_some() {
        platform.features.insert("has_custom_resolution".into());
    }
    if options.quick_play_server.is_some() {
        platform.features.insert("is_quick_play_multiplayer".into());
    }

    let vars = variables(layout, version, plan, options, platform.os);
    let sub = |s: &str| substitute(s, &vars);

    let mut args = vec![format!("-Xmx{}M", options.max_memory_mb)];
    args.extend(options.extra_jvm_args.iter().cloned());

    match &version.arguments {
        Some(arguments) => args.extend(evaluate(&arguments.jvm, &platform).iter().map(|a| sub(a))),
        None => {
            if platform.os == Os::MacOs {
                args.push("-XstartOnFirstThread".into());
            }
            args.push(sub("-Djava.library.path=${natives_directory}"));
            args.push("-cp".into());
            args.push(sub("${classpath}"));
        }
    }

    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = layout.log_config(&log.file.id);
        args.push(log.argument.replace("${path}", &path.display().to_string()));
    }

    args.push(version.main_class.clone());

    match (&version.arguments, &version.minecraft_arguments) {
        (Some(arguments), _) => args.extend(evaluate(&arguments.game, &platform).iter().map(|a| sub(a))),
        (None, Some(legacy)) => {
            args.extend(legacy.split_whitespace().map(sub));
            if let Some((w, h)) = options.resolution {
                args.extend(["--width".into(), w.to_string(), "--height".into(), h.to_string()]);
            }
        }
        (None, None) => {}
    }

    LaunchCommand { program: options.java.clone(), args, cwd: options.game_dir.clone() }
}

/// Arguments whose rules allow this platform, flattened.
fn evaluate<'a>(arguments: &'a [Argument], platform: &Platform) -> Vec<&'a str> {
    let mut out = Vec::new();
    for argument in arguments {
        match argument {
            Argument::Plain(s) => out.push(s.as_str()),
            Argument::Conditional { rules, value } if rules_allow(rules, platform) => match value {
                ArgumentValue::One(s) => out.push(s.as_str()),
                ArgumentValue::Many(v) => out.extend(v.iter().map(String::as_str)),
            },
            Argument::Conditional { .. } => {}
        }
    }
    out
}

fn variables(
    layout: &MetaLayout,
    version: &VersionInfo,
    plan: &InstallPlan,
    options: &LaunchOptions,
    os: Os,
) -> HashMap<&'static str, String> {
    let path = |p: &Path| p.display().to_string();
    let separator = if os == Os::Windows { ";" } else { ":" };
    let classpath = plan.classpath.iter().map(|p| path(p)).collect::<Vec<_>>().join(separator);
    let index_name = version
        .assets
        .clone()
        .or_else(|| version.asset_index.as_ref().map(|a| a.id.clone()))
        .unwrap_or_else(|| "legacy".into());
    let id = &options.identity;
    let (width, height) = options.resolution.unwrap_or((854, 480));

    HashMap::from([
        ("auth_player_name", id.name.clone()),
        ("auth_uuid", id.uuid.clone()),
        ("auth_access_token", id.access_token.clone()),
        ("auth_session", format!("token:{}:{}", id.access_token, id.uuid)),
        ("auth_xuid", id.xuid.clone()),
        ("user_type", id.user_type.clone()),
        ("user_properties", "{}".into()),
        ("clientid", String::new()),
        ("version_name", version.id.clone()),
        ("version_type", "Glint".into()),
        ("game_directory", path(&options.game_dir)),
        ("assets_root", path(&layout.assets())),
        ("assets_index_name", index_name),
        ("game_assets", path(&options.game_assets)),
        ("natives_directory", path(&options.natives_dir)),
        ("library_directory", path(&layout.libraries())),
        ("classpath", classpath),
        ("classpath_separator", separator.into()),
        ("launcher_name", "glint".into()),
        ("launcher_version", options.launcher_version.clone()),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
        ("quickPlayMultiplayer", options.quick_play_server.clone().unwrap_or_default()),
    ])
}

/// Replaces every `${name}` with its value; unknown names are left as they are.
fn substitute(template: &str, vars: &HashMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                match vars.get(name) {
                    Some(value) => out.push_str(value),
                    None => {
                        tracing::debug!(name, "unknown launch variable");
                        out.push_str(&rest[start..start + 2 + end + 1]);
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glint_auth::Account;
    use glint_meta::{Arch, plan_install};

    fn options() -> LaunchOptions {
        let account = Account::Offline { name: "Steve".into(), uuid: glint_auth::offline_uuid("Steve") };
        LaunchOptions {
            java: "/java/bin/java".into(),
            game_dir: "/inst/game".into(),
            natives_dir: "/inst/natives".into(),
            game_assets: "/meta/assets".into(),
            identity: account.identity(),
            max_memory_mb: 4096,
            extra_jvm_args: vec!["-XX:+UseG1GC".into()],
            resolution: None,
            quick_play_server: None,
            launcher_version: "0.1.0".into(),
        }
    }

    fn modern() -> VersionInfo {
        serde_json::from_str(
            r#"{
                "id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "assets": "19",
                "downloads": {"client": {"sha1": "c", "size": 1, "url": "u"}},
                "libraries": [{"name": "com.mojang:brigadier:1.3.10", "downloads": {"artifact": {
                    "path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar", "sha1": "b", "size": 1, "url": "u"}}}],
                "arguments": {
                    "game": ["--username", "${auth_player_name}", "--uuid", "${auth_uuid}", "--assetIndex", "${assets_index_name}",
                             {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
                              "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]},
                             {"rules": [{"action": "allow", "features": {"is_quick_play_multiplayer": true}}],
                              "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"]}],
                    "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
                            "-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
                },
                "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "type": "log4j2-xml",
                    "file": {"id": "client-1.21.2.xml", "sha1": "l", "size": 1, "url": "u"}}}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn modern_command_line() {
        let layout = MetaLayout::new("/meta");
        let version = modern();
        let platform = Platform::new(Os::Windows, Arch::X86_64);
        let plan = plan_install(&layout, &version, None, &platform, "https://res");
        let cmd = build_command(&layout, &version, &plan, &options(), &platform);
        let args = cmd.args.join(" ");

        let main = cmd.args.iter().position(|a| a == "net.minecraft.client.main.Main").unwrap();
        assert_eq!(&cmd.args[..2], ["-Xmx4096M", "-XX:+UseG1GC"]);
        assert!(!args.contains("-XstartOnFirstThread"));
        assert!(args.contains("-Dlog4j.configurationFile="));
        let cp = &cmd.args[cmd.args.iter().position(|a| a == "-cp").unwrap() + 1];
        assert!(cp.contains("brigadier-1.3.10.jar;") && cp.ends_with("1.21.4.jar"), "{cp}");
        assert_eq!(cmd.args[main + 1..], ["--username", "Steve", "--uuid", &glint_auth::offline_uuid("Steve"), "--assetIndex", "19"]);
        assert_eq!(cmd.cwd, Path::new("/inst/game"));
    }

    #[test]
    fn features_enable_resolution_and_quick_play() {
        let layout = MetaLayout::new("/meta");
        let version = modern();
        let platform = Platform::new(Os::Linux, Arch::X86_64);
        let plan = plan_install(&layout, &version, None, &platform, "https://res");
        let mut opts = options();
        opts.resolution = Some((1280, 720));
        opts.quick_play_server = Some("127.0.0.1:25565".into());

        let args = build_command(&layout, &version, &plan, &opts, &platform).args.join(" ");
        assert!(args.contains("--width 1280 --height 720"));
        assert!(args.contains("--quickPlayMultiplayer 127.0.0.1:25565"));
        assert!(args.contains(':'), "Linux classpath separator");
    }

    #[test]
    fn legacy_command_line() {
        let layout = MetaLayout::new("/meta");
        let version: VersionInfo = serde_json::from_str(
            r#"{"id": "1.8.9", "type": "release", "mainClass": "net.minecraft.client.main.Main", "assets": "1.8",
                "minecraftArguments": "--username ${auth_player_name} --session ${auth_session} --userProperties ${user_properties}"}"#,
        )
        .unwrap();
        let platform = Platform::new(Os::Windows, Arch::X86_64);
        let plan = plan_install(&layout, &version, None, &platform, "https://res");
        let cmd = build_command(&layout, &version, &plan, &options(), &platform);

        assert!(cmd.args.contains(&"-cp".to_owned()));
        assert!(cmd.args.iter().any(|a| a.starts_with("-Djava.library.path=")));
        let uuid = glint_auth::offline_uuid("Steve");
        let session = format!("token:0:{uuid}");
        assert!(cmd.args.ends_with(&[
            "--username".into(), "Steve".into(), "--session".into(), session, "--userProperties".into(), "{}".into()
        ]));
    }

    #[test]
    fn substitute_keeps_unknown_and_unterminated() {
        let vars = HashMap::from([("a", "1".to_owned())]);
        assert_eq!(substitute("x${a}y${b}z${", &vars), "x1y${b}z${");
    }
}
