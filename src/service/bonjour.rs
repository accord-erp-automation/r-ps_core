use std::fmt;

use super::mobile_contract::{APP_ID, SERVICE_ID, ServiceIdentity};

pub const BONJOUR_SERVICE_TYPE: &str = "_gscale-mobileapi._tcp.local.";
const BONJOUR_REGISTER_TYPE: &str = "_gscale-mobileapi._tcp";
const BONJOUR_DOMAIN: &str = "local.";

pub struct BonjourService {
    inner: BonjourInner,
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
        self.inner.shutdown();
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
    register_platform_bonjour(config).map(|inner| BonjourService { inner })
}

fn txt_record_bytes(properties: &[(String, String)]) -> Result<Vec<u8>, BonjourError> {
    let mut out = Vec::new();
    for (key, value) in properties {
        if key.contains('=') || !key.is_ascii() {
            return Err(BonjourError::new(format!("invalid TXT key: {key}")));
        }
        let item = format!("{key}={value}");
        if item.len() > u8::MAX as usize {
            return Err(BonjourError::new(format!("TXT item too long: {key}")));
        }
        out.push(item.len() as u8);
        out.extend_from_slice(item.as_bytes());
    }
    Ok(out)
}

#[cfg(target_os = "macos")]
struct BonjourInner {
    service_ref: apple_bonjour::DnsServiceRef,
}

#[cfg(not(target_os = "macos"))]
struct BonjourInner {
    daemon: mdns_sd::ServiceDaemon,
    fullname: String,
}

#[cfg(target_os = "macos")]
impl BonjourInner {
    fn shutdown(&mut self) {
        apple_bonjour::deallocate(self.service_ref);
        self.service_ref = std::ptr::null_mut();
    }
}

#[cfg(not(target_os = "macos"))]
impl BonjourInner {
    fn shutdown(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        self.daemon.shutdown().ok();
    }
}

#[cfg(target_os = "macos")]
fn register_platform_bonjour(config: &BonjourServiceConfig) -> Result<BonjourInner, BonjourError> {
    let txt = txt_record_bytes(&config.properties)?;
    let service_ref = apple_bonjour::register(
        &config.instance_name,
        BONJOUR_REGISTER_TYPE,
        BONJOUR_DOMAIN,
        &config.host_name,
        config.port,
        &txt,
    )?;
    Ok(BonjourInner { service_ref })
}

#[cfg(not(target_os = "macos"))]
fn register_platform_bonjour(config: &BonjourServiceConfig) -> Result<BonjourInner, BonjourError> {
    use mdns_sd::{ServiceDaemon, ServiceInfo};

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

    Ok(BonjourInner { daemon, fullname })
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

    #[test]
    fn encodes_dns_sd_txt_records() {
        let txt = txt_record_bytes(&[
            ("service".to_string(), "mobileapi".to_string()),
            ("app".to_string(), "gscale-zebra".to_string()),
        ])
        .unwrap();

        assert_eq!(txt, b"\x11service=mobileapi\x10app=gscale-zebra".to_vec());
    }
}

#[cfg(target_os = "macos")]
mod apple_bonjour {
    use std::ffi::{CString, c_char, c_void};
    use std::ptr;

    use super::BonjourError;

    pub type DnsServiceRef = *mut c_void;
    type DnsServiceFlags = u32;
    type DnsServiceErrorType = i32;

    const DNS_SERVICE_ERR_NO_ERROR: DnsServiceErrorType = 0;

    #[link(name = "System")]
    unsafe extern "C" {
        fn DNSServiceRegister(
            sd_ref: *mut DnsServiceRef,
            flags: DnsServiceFlags,
            interface_index: u32,
            name: *const c_char,
            regtype: *const c_char,
            domain: *const c_char,
            host: *const c_char,
            port: u16,
            txt_len: u16,
            txt_record: *const c_void,
            callback: Option<extern "C" fn()>,
            context: *mut c_void,
        ) -> DnsServiceErrorType;

        fn DNSServiceRefDeallocate(sd_ref: DnsServiceRef);
    }

    pub fn register(
        instance_name: &str,
        regtype: &str,
        domain: &str,
        host_name: &str,
        port: u16,
        txt: &[u8],
    ) -> Result<DnsServiceRef, BonjourError> {
        if txt.len() > u16::MAX as usize {
            return Err(BonjourError::new("Bonjour TXT record is too large"));
        }

        let instance_name = cstring("instance name", instance_name)?;
        let regtype = cstring("register type", regtype)?;
        let domain = cstring("domain", domain)?;
        let host_name = cstring("host name", host_name)?;
        let mut service_ref = ptr::null_mut();
        let txt_ptr = if txt.is_empty() {
            ptr::null()
        } else {
            txt.as_ptr().cast::<c_void>()
        };

        let err = unsafe {
            DNSServiceRegister(
                &mut service_ref,
                0,
                0,
                instance_name.as_ptr(),
                regtype.as_ptr(),
                domain.as_ptr(),
                host_name.as_ptr(),
                port.to_be(),
                txt.len() as u16,
                txt_ptr,
                None,
                ptr::null_mut(),
            )
        };
        if err != DNS_SERVICE_ERR_NO_ERROR {
            return Err(BonjourError::new(format!(
                "DNSServiceRegister failed: {err}"
            )));
        }
        if service_ref.is_null() {
            return Err(BonjourError::new("DNSServiceRegister returned null"));
        }
        Ok(service_ref)
    }

    pub fn deallocate(service_ref: DnsServiceRef) {
        if !service_ref.is_null() {
            unsafe {
                DNSServiceRefDeallocate(service_ref);
            }
        }
    }

    fn cstring(label: &str, value: &str) -> Result<CString, BonjourError> {
        CString::new(value).map_err(|_| BonjourError::new(format!("{label} contains NUL byte")))
    }
}
