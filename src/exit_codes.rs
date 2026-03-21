// Exit codes for balebos commands
// These codes can be used by scripts and automation to respond appropriately

// Success
pub const EXIT_SUCCESS: i32 = 0;

// General errors
pub const EXIT_GENERAL_ERROR: i32 = 1;
pub const EXIT_INVALID_ARGUMENT: i32 = 2;
pub const EXIT_STORAGE_ERROR: i32 = 3;

// Domain-related errors
pub const EXIT_DOMAIN_NOT_FOUND: i32 = 11;

// Check/report specific
pub const EXIT_CRITICAL_ISSUES: i32 = 20;
pub const EXIT_WARNING_ISSUES: i32 = 21;

// HTTP probe specific
pub const EXIT_HTTP_ERROR: i32 = 30;
pub const EXIT_HTTP_CLIENT_ERROR: i32 = 31;
pub const EXIT_HTTP_SERVER_ERROR: i32 = 32;

// Subdomain probe specific
pub const EXIT_PROBE_ERROR: i32 = 40;
pub const EXIT_EXTERNAL_TOOL_ERROR: i32 = 41;
