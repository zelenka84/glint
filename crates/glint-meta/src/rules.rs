//! Mojang's allow/disallow rules, used by libraries and launch arguments.

use std::collections::{BTreeSet, HashMap};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// Regex matched against the OS version, e.g. `^10\.`.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacOs,
}

impl Os {
    /// The name Mojang uses in rules and `natives` maps.
    pub fn mojang_name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOs => "osx",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86,
    X86_64,
    Arm64,
}

impl Arch {
    /// The name Mojang uses in `os.arch` rules. `x86` means a 32-bit system.
    pub fn mojang_name(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "x86_64",
            Self::Arm64 => "arm64",
        }
    }

    /// Replacement for `${arch}` in legacy native classifiers.
    pub fn bits(self) -> &'static str {
        match self {
            Self::X86 => "32",
            Self::X86_64 | Self::Arm64 => "64",
        }
    }
}

/// The machine a version is being installed or launched on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
    /// OS version string matched by `os.version` rules (e.g. `10.0` on Windows 10/11).
    pub os_version: String,
    /// Enabled launcher features such as `has_custom_resolution` or `is_demo_user`.
    pub features: BTreeSet<String>,
}

impl Platform {
    pub fn new(os: Os, arch: Arch) -> Self {
        Self {
            os,
            arch,
            os_version: String::new(),
            features: BTreeSet::new(),
        }
    }

    /// The platform Glint is running on.
    pub fn current() -> Self {
        let os = if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        };
        let arch = if cfg!(target_arch = "aarch64") {
            Arch::Arm64
        } else if cfg!(target_arch = "x86") {
            Arch::X86
        } else {
            Arch::X86_64
        };
        // Windows 10 and 11 both report 10.0; the only rule that reads this targets Windows 10+.
        let os_version = if os == Os::Windows {
            "10.0".to_owned()
        } else {
            String::new()
        };
        Self {
            os,
            arch,
            os_version,
            features: BTreeSet::new(),
        }
    }

    pub fn with_feature(mut self, feature: &str) -> Self {
        self.features.insert(feature.to_owned());
        self
    }
}

impl Rule {
    fn matches(&self, platform: &Platform) -> bool {
        if let Some(os) = &self.os {
            if os
                .name
                .as_deref()
                .is_some_and(|n| n != platform.os.mojang_name())
            {
                return false;
            }
            if os
                .arch
                .as_deref()
                .is_some_and(|a| a != platform.arch.mojang_name())
            {
                return false;
            }
            if let Some(pattern) = &os.version {
                let ok =
                    regex::Regex::new(pattern).is_ok_and(|re| re.is_match(&platform.os_version));
                if !ok {
                    return false;
                }
            }
        }
        self.features
            .iter()
            .all(|(name, wanted)| platform.features.contains(name) == *wanted)
    }
}

/// Mojang semantics: no rules means allowed; otherwise the last matching rule wins
/// and nothing matching means disallowed.
pub fn rules_allow(rules: &[Rule], platform: &Platform) -> bool {
    if rules.is_empty() {
        return true;
    }
    rules
        .iter()
        .rev()
        .find(|r| r.matches(platform))
        .is_some_and(|r| r.action == RuleAction::Allow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn empty_rules_allow() {
        assert!(rules_allow(&[], &Platform::new(Os::Linux, Arch::X86_64)));
    }

    #[test]
    fn allow_all_except_osx() {
        let r = rules(r#"[{"action": "allow"}, {"action": "disallow", "os": {"name": "osx"}}]"#);
        assert!(rules_allow(&r, &Platform::new(Os::Windows, Arch::X86_64)));
        assert!(!rules_allow(&r, &Platform::new(Os::MacOs, Arch::Arm64)));
    }

    #[test]
    fn only_on_named_os_and_arch() {
        let r = rules(r#"[{"action": "allow", "os": {"name": "windows", "arch": "x86"}}]"#);
        assert!(rules_allow(&r, &Platform::new(Os::Windows, Arch::X86)));
        assert!(!rules_allow(&r, &Platform::new(Os::Windows, Arch::X86_64)));
        assert!(!rules_allow(&r, &Platform::new(Os::Linux, Arch::X86)));
    }

    #[test]
    fn os_version_regex() {
        let r = rules(r#"[{"action": "allow", "os": {"name": "windows", "version": "^10\\."}}]"#);
        assert!(rules_allow(
            &r,
            &Platform::current_like(Os::Windows, "10.0")
        ));
        assert!(!rules_allow(
            &r,
            &Platform::current_like(Os::Windows, "6.1")
        ));
    }

    #[test]
    fn features_must_match() {
        let r = rules(r#"[{"action": "allow", "features": {"is_demo_user": true}}]"#);
        let p = Platform::new(Os::Windows, Arch::X86_64);
        assert!(!rules_allow(&r, &p));
        assert!(rules_allow(&r, &p.with_feature("is_demo_user")));
    }

    impl Platform {
        fn current_like(os: Os, version: &str) -> Self {
            Self {
                os_version: version.to_owned(),
                ..Self::new(os, Arch::X86_64)
            }
        }
    }
}
