pub mod bonjour;
pub mod config;
pub mod discovery;
pub mod discovery_runtime;
pub mod driver_print_contract;
pub mod driver_print_runtime;
pub mod http;
pub mod http_runtime;
pub mod mobile_contract;
pub mod monitor_contract;
pub mod monitor_runtime;
pub mod print_activity;

pub use bonjour::{
    BONJOUR_SERVICE_TYPE, BonjourError, BonjourService, BonjourServiceConfig, bonjour_config,
    register_bonjour_service,
};
pub use config::{
    DEFAULT_DISCOVERY_PORT, DEFAULT_MOBILE_API_PORTS, MobileServiceConfig, default_mobile_api_port,
    parse_candidate_ports, select_listen_addr, server_ref_for_port,
};
pub use discovery::{
    DISCOVERY_ANNOUNCE_INTERVAL_MS, DISCOVERY_PROBE_V1, DiscoverySocketConfig,
    bind_announcement_socket, collect_discovery_broadcast_targets, discovery_response_for_packet,
};
pub use discovery_runtime::{DiscoveryRuntimeState, serve_discovery, serve_discovery_socket};
pub use driver_print_contract::{
    DriverPrintErrorResponse, DriverPrintJob, DriverPrintRequest, DriverPrintRequestError,
    DriverPrintResponse,
};
pub use driver_print_runtime::{
    DeviceDriverPrintExecutor, DriverPrintExecutionError, DriverPrintExecutor, PrintExecutorMode,
    SimulatedDriverPrintExecutor, UnconfiguredDriverPrintExecutor, device_executor_from_env,
    print_executor_mode_from_env, simulated_executor_from_env,
};
pub use http::{
    MobileHttpResponse, MobileHttpState, handle_mobile_http_request,
    handle_mobile_http_request_with_body,
};
pub use http_runtime::{
    bind_mobile_http_listener, handle_mobile_http_stream, route_raw_http_request, serve_mobile_http,
};
pub use mobile_contract::{
    APP_ID, ActivePrinterResponse, DiscoveryAnnouncement, EmptyArchiveResponse, EmptyItemsResponse,
    EmptyWarehousesResponse, HandshakeResponse, HealthResponse, ItemWarehousesResponse,
    PrinterCapabilitiesResponse, PrinterCapabilityFlagsResponse, SERVICE_ID, ServiceIdentity,
    SetupStatusResponse,
};
pub use monitor_contract::{BatchStateResponse, MonitorResponse, MonitorState};
pub use monitor_runtime::MonitorRuntimeState;
pub use print_activity::{PrintActivitySnapshot, PrintActivityState};
