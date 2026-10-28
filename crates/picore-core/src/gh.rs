use std::path::Path;

use crate::process::{self, ProcError, TIMEOUT};

pub fn current_user() -> Result<String, ProcError> {
    process::run(
        "gh",
        Path::new("."),
        &["api", "user", "--jq", ".login"],
        TIMEOUT,
    )
}
