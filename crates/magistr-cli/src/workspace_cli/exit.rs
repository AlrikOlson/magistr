//! Process-level refusals of the workspace verbs: answered before any
//! envelope exists, with the retained exit code.

/// EX_USAGE: a malformed flag or an unusable target.
pub(crate) const EXIT_USAGE: i32 = 64;

/// Print the retained diagnostic and exit with `code`.
pub(crate) fn fail(code: i32, message: &str) -> ! {
    eprintln!("magistr workspace: {message}");
    std::process::exit(code)
}
