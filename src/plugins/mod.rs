//! Sub-plugin modules. Add or remove entries in `build_registry` only.

pub mod agents;
pub mod board;
pub mod bootstrap;
pub mod config;
pub mod connect;
pub mod docs;
pub mod doctor;
pub mod git;
pub mod search;
pub mod secrets;
pub mod workspace;

use crate::registry::PluginRegistry;

/// Single place to enable/disable sub-plugins.
pub fn build_registry() -> PluginRegistry {
    let mut reg = PluginRegistry::new();
    reg.register(Box::new(docs::DocsPlugin::new()));
    reg.register(Box::new(git::GitPlugin::new()));
    reg.register(Box::new(workspace::WorkspacePlugin::new()));
    reg.register(Box::new(board::BoardPlugin::new()));
    reg.register(Box::new(secrets::SecretsPlugin::new()));
    reg.register(Box::new(connect::ConnectPlugin::new()));
    reg.register(Box::new(agents::AgentsPlugin::new()));
    reg.register(Box::new(bootstrap::BootstrapPlugin::new()));
    reg.register(Box::new(search::SearchPlugin::new()));
    reg.register(Box::new(doctor::DoctorPlugin::new()));
    reg.register(Box::new(config::ConfigPlugin::new()));
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_expected_plugins_registered() {
        let reg = build_registry();
        let ids = reg.ids();
        for expected in [
            "docs",
            "git",
            "workspace",
            "board",
            "secrets",
            "connect",
            "agents",
            "bootstrap",
            "search",
            "doctor",
            "config",
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
        assert_eq!(ids.len(), 11);
    }
}
