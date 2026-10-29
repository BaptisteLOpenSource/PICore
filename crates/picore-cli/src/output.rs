use std::io::{self, IsTerminal};
use std::process::ExitCode;

use serde::Serialize;

const SCHEMA: u8 = 1;

#[derive(Serialize)]
struct Success<'a, T: Serialize> {
    schema: u8,
    data: &'a T,
}

#[derive(Serialize)]
struct Failure<'a> {
    schema: u8,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    candidates: &'a [String],
}

pub fn emit<T: Serialize>(json: bool, data: &T, human: impl FnOnce(&T)) {
    if json {
        let line = serde_json::to_string(&Success {
            schema: SCHEMA,
            data,
        })
        .expect("data serializes");
        println!("{line}");
    } else {
        human(data);
    }
}

pub fn fail(json: bool, code: &str, message: &str, candidates: &[String]) -> ExitCode {
    if json {
        let body = Failure {
            schema: SCHEMA,
            error: ErrorBody {
                code,
                message,
                candidates,
            },
        };
        println!(
            "{}",
            serde_json::to_string(&body).expect("error serializes")
        );
    } else {
        eprintln!("error: {message}");
        for c in candidates {
            eprintln!("  {c}");
        }
    }
    ExitCode::from(exit_code(code))
}

fn exit_code(code: &str) -> u8 {
    match code {
        "ambiguous" => 3,
        "not_found" => 4,
        _ => 1,
    }
}

pub fn mascot(json: bool, quiet: bool, line: &str) {
    if !json && !quiet && io::stderr().is_terminal() {
        eprintln!("(o> {line}");
    }
}
