//! Selection of the AmneziaWG interface the panel manages.
//!
//! The installer supports one interface per host and records its name as
//! `SERVER_AWG_NIC` in the params file.  A host may still carry further,
//! hand-made AWG interfaces (for example `awg1` next to the managed `awg0`),
//! and `awg show all dump` reports all of them.  Without scoping, the poller
//! would record those foreign peers as if they were remnants of the panel's
//! own interface, and the enforcement helpers would act on interfaces the
//! panel does not own.
//!
//! [`InterfaceSelector`] resolves the managed interface name, either from the
//! optional `AWG_INTERFACE` override or from the installer params, and
//! [`scope_to_interface`] narrows a parsed dump to that interface.

use crate::admin::client_manager::{parse_params, validate_interface_name_for};
use crate::awg::{self, AwgInterface};

/// Errors raised while determining the managed interface.
#[derive(Debug, thiserror::Error)]
pub enum ManagedInterfaceError {
    /// `AWG_INTERFACE` is set but is not a safe interface name.
    #[error("{0}")]
    InvalidOverride(String),

    /// The installer params could not be read through the privileged helper.
    #[error("failed to read installer params: {0}")]
    ParamsRead(#[source] awg::AwgError),

    /// The installer params do not describe a usable interface.
    #[error("failed to parse installer params: {0}")]
    ParamsParse(String),

    /// The blocking resolution task panicked or was cancelled.
    #[error("interface resolution task failed: {0}")]
    Task(String),
}

/// Decides which AWG interface the panel monitors and manages.
///
/// Cloning is cheap: the selector only stores the optional override.  The
/// params are re-read on every [`resolve`](Self::resolve), so an interface
/// renamed by re-running the installer is picked up without a restart.
#[derive(Clone, Debug, Default)]
pub struct InterfaceSelector {
    /// Interface name pinned by `AWG_INTERFACE`; `None` defers to the params.
    override_name: Option<String>,
}

impl InterfaceSelector {
    /// Follow `SERVER_AWG_NIC` from the installer params.
    pub fn from_params() -> Self {
        Self::default()
    }

    /// Build a selector from the optional `AWG_INTERFACE` value.
    ///
    /// Surrounding whitespace is ignored and an empty value counts as unset,
    /// so a blank `AWG_INTERFACE=` line in the env file falls back to the
    /// params.  Any other value must pass the same validation as
    /// `SERVER_AWG_NIC`, because it is handed to the privileged helper.
    pub fn from_override(value: Option<&str>) -> Result<Self, ManagedInterfaceError> {
        let name = match value.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => name,
            None => return Ok(Self::from_params()),
        };
        validate_interface_name_for("AWG_INTERFACE", name)
            .map_err(ManagedInterfaceError::InvalidOverride)?;
        Ok(Self {
            override_name: Some(name.to_string()),
        })
    }

    /// The `AWG_INTERFACE` override, if one is set.
    pub fn override_name(&self) -> Option<&str> {
        self.override_name.as_deref()
    }

    /// Where the managed interface name comes from, for log lines.
    pub fn source(&self) -> &'static str {
        if self.override_name.is_some() {
            "AWG_INTERFACE"
        } else {
            "SERVER_AWG_NIC"
        }
    }

    /// Resolve the managed interface name.
    ///
    /// Without an override this reads the installer params through the
    /// privileged helper – a blocking `sudo` call – so run it off the async
    /// executor.
    pub fn resolve(&self) -> Result<String, ManagedInterfaceError> {
        self.resolve_with(awg::read_params_via_sudo)
    }

    /// [`resolve`](Self::resolve) with an injectable params reader.
    pub fn resolve_with<F>(&self, read_params: F) -> Result<String, ManagedInterfaceError>
    where
        F: FnOnce() -> Result<String, awg::AwgError>,
    {
        if let Some(name) = &self.override_name {
            return Ok(name.clone());
        }
        let content = read_params().map_err(ManagedInterfaceError::ParamsRead)?;
        let params = parse_params(&content)
            .map_err(|e| ManagedInterfaceError::ParamsParse(e.to_string()))?;
        Ok(params.server_awg_nic)
    }
}

/// A parsed dump narrowed to the managed interface.
#[derive(Debug, Default)]
pub struct ScopedInterfaces {
    /// The managed interface when the dump contained it (at most one entry).
    pub matched: Vec<AwgInterface>,
    /// Names of the other interfaces in the dump, sorted.
    pub ignored: Vec<String>,
}

/// Keep only the interface named `managed`, reporting the others by name.
pub fn scope_to_interface(interfaces: Vec<AwgInterface>, managed: &str) -> ScopedInterfaces {
    let mut scoped = ScopedInterfaces::default();
    for iface in interfaces {
        if iface.name == managed {
            scoped.matched.push(iface);
        } else {
            scoped.ignored.push(iface.name);
        }
    }
    scoped.ignored.sort();
    scoped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PublicKey;

    /// Minimal AWG 2.0 params as serialized by the installer.
    const PARAMS_AWG0: &str = "SERVER_PUB_IP='198.51.100.1'\nSERVER_AWG_NIC='awg0'\nSERVER_AWG_IPV4='10.66.66.1'\nSERVER_AWG_IPV6='fd42:42:42::1'\nSERVER_PORT='51820'\nSERVER_PUB_KEY='PUB'\nCLIENT_DNS_1='1.1.1.1'\nALLOWED_IPS='0.0.0.0/0, ::/0'\nSERVER_AWG_JC='4'\nSERVER_AWG_JMIN='50'\nSERVER_AWG_JMAX='1000'\nSERVER_AWG_S1='30'\nSERVER_AWG_S2='100'\nSERVER_AWG_S3='45'\nSERVER_AWG_S4='120'\nSERVER_AWG_H1='5-10'\nSERVER_AWG_H2='11-20'\nSERVER_AWG_H3='21-30'\nSERVER_AWG_H4='31-40'\n";

    fn iface(name: &str, peer_keys: &[&str]) -> AwgInterface {
        AwgInterface {
            name: name.to_string(),
            public_key: PublicKey(format!("{name}_SERVER_KEY=")),
            listen_port: Some(51820),
            peers: peer_keys
                .iter()
                .map(|key| awg::AwgPeer {
                    public_key: PublicKey((*key).to_string()),
                    endpoint: None,
                    allowed_ips: Vec::new(),
                    last_handshake: None,
                    rx_bytes: 0,
                    tx_bytes: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn override_wins_without_reading_params() {
        let selector = InterfaceSelector::from_override(Some("awg1")).expect("valid override");
        assert_eq!(selector.override_name(), Some("awg1"));
        assert_eq!(selector.source(), "AWG_INTERFACE");

        let resolved = selector
            .resolve_with(|| panic!("params must not be read when AWG_INTERFACE is set"))
            .expect("resolve override");
        assert_eq!(resolved, "awg1");
    }

    #[test]
    fn override_is_trimmed_and_blank_means_unset() {
        let trimmed = InterfaceSelector::from_override(Some("  awg1\n")).expect("trimmed");
        assert_eq!(trimmed.override_name(), Some("awg1"));

        for blank in [None, Some(""), Some("   ")] {
            let selector = InterfaceSelector::from_override(blank).expect("blank is unset");
            assert!(selector.override_name().is_none());
            assert_eq!(selector.source(), "SERVER_AWG_NIC");
        }
    }

    #[test]
    fn override_is_validated_like_server_awg_nic() {
        for invalid in [
            "-awg0",
            "../etc",
            "awg0/conf",
            "awg 0",
            "1234567890123456",
            "awg0;id",
        ] {
            let error = InterfaceSelector::from_override(Some(invalid)).expect_err(invalid);
            assert!(
                matches!(error, ManagedInterfaceError::InvalidOverride(_)),
                "{invalid}: {error}"
            );
            assert!(
                error.to_string().contains("AWG_INTERFACE"),
                "{invalid}: {error}"
            );
        }
    }

    #[test]
    fn params_provide_the_interface_when_no_override_is_set() {
        let selector = InterfaceSelector::from_params();
        let resolved = selector
            .resolve_with(|| Ok(PARAMS_AWG0.to_string()))
            .expect("resolve from params");
        assert_eq!(resolved, "awg0");
    }

    #[test]
    fn params_read_failure_is_reported() {
        let selector = InterfaceSelector::from_params();
        let error = selector
            .resolve_with(|| {
                Err(awg::AwgError::NonZeroExit {
                    status: 1,
                    stderr: "sudo: a password is required".into(),
                })
            })
            .expect_err("read failure");
        assert!(matches!(error, ManagedInterfaceError::ParamsRead(_)));
        assert!(error.to_string().contains("a password is required"));
    }

    #[test]
    fn unparseable_params_are_reported() {
        let selector = InterfaceSelector::from_params();

        let missing_keys = selector
            .resolve_with(|| Ok("SERVER_AWG_NIC='awg0'\n".to_string()))
            .expect_err("missing keys");
        assert!(matches!(
            missing_keys,
            ManagedInterfaceError::ParamsParse(_)
        ));

        let unsafe_nic = selector
            .resolve_with(|| {
                Ok(PARAMS_AWG0.replace("SERVER_AWG_NIC='awg0'", "SERVER_AWG_NIC='../etc'"))
            })
            .expect_err("unsafe nic");
        assert!(matches!(unsafe_nic, ManagedInterfaceError::ParamsParse(_)));
    }

    #[test]
    fn scope_keeps_only_the_managed_interface() {
        let dump = vec![
            iface("awg1", &["FOREIGN="]),
            iface("awg0", &["A=", "B="]),
            iface("wg0", &[]),
        ];
        let scoped = scope_to_interface(dump, "awg0");
        assert_eq!(scoped.matched.len(), 1);
        assert_eq!(scoped.matched[0].name, "awg0");
        assert_eq!(scoped.matched[0].peers.len(), 2);
        assert_eq!(scoped.ignored, vec!["awg1".to_string(), "wg0".to_string()]);
    }

    #[test]
    fn scope_reports_a_missing_managed_interface() {
        let scoped = scope_to_interface(vec![iface("awg1", &["FOREIGN="])], "awg0");
        assert!(scoped.matched.is_empty());
        assert_eq!(scoped.ignored, vec!["awg1".to_string()]);

        let empty = scope_to_interface(Vec::new(), "awg0");
        assert!(empty.matched.is_empty());
        assert!(empty.ignored.is_empty());
    }
}
