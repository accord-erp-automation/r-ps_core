use std::fmt;

use mdns_sd::{ServiceDaemon, ServiceInfo};

use super::mobile_contract::{APP_ID, SERVICE_ID, ServiceIdentity};

pub const BONJOUR_SERVICE_TYPE: &str = "_gscale-mobileapi._tcp.local.";

pub struct BonjourService {
    daemon: ServiceDaemon,
    fullname: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BonjourServiceConfig {
    pub instance_name: String,
    pub host_name: String,
    pub port: u16,
    pub properties: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BonjourError {
    message: String,
}

impl BonjourError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for BonjourError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BonjourError {}

impl Drop for BonjourService {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        self.daemon.shutdown().ok();
    }
}

pub fn bonjour_config(
    identity: &ServiceIdentity,
    server_name: &str,
    port: u16,
) -> BonjourServiceConfig {
    let instance_name = normalize_bonjour_text(server_name, APP_ID);
    let host_name = format!("{}.local.", trim_bonjour_host_name(server_name));
    let properties = vec![
        ("service".to_string(), SERVICE_ID.to_string()),
        ("app".to_string(), APP_ID.to_string()),
        (
            "server_name".to_string(),
            normalize_bonjour_text(&identity.server_name, APP_ID),
        ),
        (
            "server_ref".to_string(),
            normalize_bonjour_text(&identity.server_ref, "unknown"),
        ),
        (
            "display_name".to_string(),
            normalize_bonjour_text(&identity.display_name, "Operator"),
        ),
        (
            "role".to_string(),
            normalize_bonjour_text(&identity.role, "operator"),
        ),
        ("http_port".to_string(), port.to_string()),
    ];

    BonjourServiceConfig {
        instance_name,
        host_name,
        port,
        properties,
    }
}

pub fn register_bonjour_service(
    config: &BonjourServiceConfig,
) -> Result<BonjourService, BonjourError> {
    let daemon =
        ServiceDaemon::new().map_err(|err| BonjourError::new(format!("bonjour daemon: {err}")))?;
    let property_refs = config
        .properties
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let service_info = ServiceInfo::new(
        BONJOUR_SERVICE_TYPE,
        &config.instance_name,
        &config.host_name,
        "",
        config.port,
        property_refs.as_slice(),
    )
    .map_err(|err| BonjourError::new(format!("bonjour service info: {err}")))?
    .enable_addr_auto();
    let fullname = service_info.get_fullname().to_string();

    daemon
        .register(service_info)
        .map_err(|err| BonjourError::new(format!("bonjour register: {err}")))?;

    Ok(BonjourService { daemon, fullname })
}

fn normalize_bonjour_text(value: &str, fallback: &str) -> String {
    match value.trim() {
        "" => fallback.to_string(),
        value => value.replace(['\n', '\r'], " "),
    }
}

fn trim_bonjour_host_name(value: &str) -> String {
    let value = value.trim().trim_end_matches(".local.");
    let value = value.trim_end_matches(".local").trim_matches('.');
    match value {
        "" => APP_ID.to_string(),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_gscale_compatible_bonjour_config() {
        let identity = ServiceIdentity::new("rp-scale", "dev-operator", "Operator One", "admin");
        let config = bonjour_config(&identity, "rp-scale.local", 39117);

        assert_eq!(BONJOUR_SERVICE_TYPE, "_gscale-mobileapi._tcp.local.");
        assert_eq!(config.instance_name, "rp-scale.local");
        assert_eq!(config.host_name, "rp-scale.local.");
        assert_eq!(config.port, 39117);
        assert!(
            config
                .properties
                .contains(&("service".to_string(), "mobileapi".to_string()))
        );
        assert!(
            config
                .properties
                .contains(&("app".to_string(), "gscale-zebra".to_string()))
        );
        assert!(
            config
                .properties
                .contains(&("role".to_string(), "admin".to_string()))
        );
    }

    #[test]
    fn trims_bonjour_hostname_like_gscale() {
        assert_eq!(trim_bonjour_host_name("gscale.local."), "gscale");
        assert_eq!(trim_bonjour_host_name(""), "gscale-zebra");
    }
}
