/// The closed core tool surface: seven tools plus undo, plus the status tool.
/// No plugin may occupy or mimic any of these bare names; plugin wire names
/// are derived as `<publisher>.<pkg>/<tool>` and always contain a slash.
pub const RESERVED_TOOL_NAMES: [&str; 9] = [
    "search",
    "read",
    "edit",
    "map",
    "files",
    "script",
    "run",
    "undo",
    "workspace_status",
];
