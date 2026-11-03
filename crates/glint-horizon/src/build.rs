//! One installed Horizon build: asking what it supports and preparing its resources.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{Error, Result};

/// `name` with `.exe` on Windows.
pub(crate) fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// A command that opens no console window on Windows.
fn command(program: &Path, cwd: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.current_dir(cwd).stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    /// Folder name, e.g. `nightly-2026-11-03-f42ef42`.
    pub id: String,
    /// The release title, e.g. `Nightly 2026-11-03 · f42ef42`.
    pub name: String,
    /// Holds `horizon-client`, `horizon-prepare`, `research-lock.json` and `exporters/`.
    pub dir: PathBuf,
    resources: PathBuf,
}

/// The reply of `horizon-client --launch-api`. Unknown fields are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct LaunchApi {
    pub launch_api: String,
    #[serde(default)]
    pub horizon: Option<String>,
    #[serde(default)]
    pub minecraft: Option<String>,
    #[serde(default)]
    pub options: Vec<LaunchOption>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchOption {
    pub name: String,
}

impl LaunchApi {
    /// Options a build does not list are refused with exit code 2, so pass only these.
    pub fn supports(&self, option: &str) -> bool {
        self.options.iter().any(|o| o.name == option)
    }
}

/// The player's Minecraft files `horizon-prepare` reads, plus the Java it runs.
#[derive(Debug, Clone)]
pub struct PrepareInputs {
    pub version_json: PathBuf,
    pub client_jar: PathBuf,
    /// Maven layout.
    pub libraries_dir: PathBuf,
    /// Holds `indexes/` and `objects/`.
    pub assets_dir: PathBuf,
    pub server_jar: PathBuf,
    /// The Java home (not the executable) of the version the preparer asks for.
    pub java_home: PathBuf,
}

/// A line of `horizon-prepare --progress json` output.
#[derive(Debug, Clone, PartialEq)]
pub enum PrepareEvent {
    Message(String),
    /// From 0 to 1, never decreasing.
    Fraction(f64),
}

#[derive(Deserialize)]
struct ProgressLine {
    message: Option<String>,
    fraction: Option<f64>,
    error: Option<String>,
}

impl Build {
    pub fn new(id: &str, name: &str, dir: PathBuf, resources: PathBuf) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            dir,
            resources,
        }
    }

    pub fn client(&self) -> PathBuf {
        self.dir.join(exe("horizon-client"))
    }

    pub fn preparer(&self) -> PathBuf {
        self.dir.join(exe("horizon-prepare"))
    }

    /// What `--resources` gets: the prepared folder, shared by every instance of this build.
    pub fn resources(&self) -> &Path {
        &self.resources
    }

    /// Asks the game which Launch API and options it supports. Only 0.1.x is understood.
    pub async fn launch_api(&self) -> Result<LaunchApi> {
        let program = self.client();
        let output = command(&program, &self.dir)
            .arg("--launch-api")
            .output()
            .await
            .map_err(|source| Error::Run {
                path: program.clone(),
                source,
            })?;
        let protocol = |message: String| Error::Protocol {
            program: "horizon-client --launch-api".into(),
            message,
        };
        if !output.status.success() {
            return Err(protocol(format!(
                "{}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let api: LaunchApi =
            serde_json::from_slice(&output.stdout).map_err(|e| protocol(e.to_string()))?;
        if api.launch_api != "0.1" && !api.launch_api.starts_with("0.1.") {
            return Err(Error::LaunchApiVersion(api.launch_api));
        }
        Ok(api)
    }

    /// True when the resources are complete and current for this build. Fast; run it
    /// before every start.
    pub async fn is_prepared(&self) -> Result<bool> {
        if !self.resources.is_dir() {
            return Ok(false);
        }
        let program = self.preparer();
        let output = command(&program, &self.dir)
            .arg("--check")
            .arg("--output")
            .arg(&self.resources)
            .output()
            .await
            .map_err(|source| Error::Run {
                path: program.clone(),
                source,
            })?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Error::Protocol {
                program: "horizon-prepare --check".into(),
                message: format!(
                    "{}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            }),
        }
    }

    /// Prepares the resources from the player's files. The first time takes minutes;
    /// later runs skip what is already current.
    pub async fn prepare(&self, inputs: &PrepareInputs, on: impl Fn(PrepareEvent)) -> Result<()> {
        let program = self.preparer();
        tokio::fs::create_dir_all(&self.resources)
            .await
            .map_err(|e| crate::io(&self.resources, e))?;
        let mut child = command(&program, &self.dir)
            .arg("--prepare")
            .arg("--output")
            .arg(&self.resources)
            .args(["--complete", "--progress", "json"])
            .arg("--version-json")
            .arg(&inputs.version_json)
            .arg("--client-jar")
            .arg(&inputs.client_jar)
            .arg("--libraries-dir")
            .arg(&inputs.libraries_dir)
            .arg("--assets-dir")
            .arg(&inputs.assets_dir)
            .arg("--server-jar")
            .arg(&inputs.server_jar)
            .arg("--java-home")
            .arg(&inputs.java_home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| Error::Run {
                path: program.clone(),
                source,
            })?;

        // stderr is human text: kept for the error message and shown as it comes.
        let stderr = child.stderr.take().expect("stderr is piped");
        let (tx, mut stderr_lines) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut stdout = BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
        let mut error = None;
        let mut last_stderr = None;
        loop {
            tokio::select! {
                line = stdout.next_line() => match line {
                    Ok(Some(line)) => match serde_json::from_str::<ProgressLine>(&line) {
                        Ok(progress) => {
                            if let Some(message) = progress.message {
                                on(PrepareEvent::Message(message));
                            }
                            if let Some(fraction) = progress.fraction {
                                on(PrepareEvent::Fraction(fraction.clamp(0.0, 1.0)));
                            }
                            if progress.error.is_some() {
                                error = progress.error;
                            }
                        }
                        Err(_) if !line.trim().is_empty() => on(PrepareEvent::Message(line)),
                        Err(_) => {}
                    },
                    _ => break,
                },
                Some(line) = stderr_lines.recv() => {
                    if !line.trim().is_empty() {
                        on(PrepareEvent::Message(line.clone()));
                        last_stderr = Some(line);
                    }
                }
            }
        }
        while let Some(line) = stderr_lines.recv().await {
            if !line.trim().is_empty() {
                on(PrepareEvent::Message(line.clone()));
                last_stderr = Some(line);
            }
        }
        let status = child.wait().await.map_err(|source| Error::Run {
            path: program.clone(),
            source,
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Prepare(error.or(last_stderr).unwrap_or_else(|| {
                format!("horizon-prepare exited with {status}")
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_api_reply_is_read() {
        let api: LaunchApi = serde_json::from_str(
            r#"{"crash_report_line": "Horizon: crash report saved to ", "exit_codes": [0, 1, 2, 101],
                "horizon": "nightly", "launch_api": "0.1.0", "minecraft": "26.3",
                "options": [{"name": "--resources", "value": "DIR"}, {"name": "--fullscreen", "value": null}]}"#,
        )
        .unwrap();
        assert_eq!(api.minecraft.as_deref(), Some("26.3"));
        assert!(api.supports("--fullscreen"));
        assert!(!api.supports("--width"));
    }

    #[test]
    fn progress_lines_are_read() {
        let line: ProgressLine = serde_json::from_str(r#"{"fraction": 0.5, "extra": 1}"#).unwrap();
        assert_eq!(line.fraction, Some(0.5));
        assert!(line.message.is_none() && line.error.is_none());
    }
}
