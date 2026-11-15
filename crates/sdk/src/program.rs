//! What a pane runs, and what steg knows about it right now (doc-15 R97).

use serde::{Deserialize, Serialize};

/// What a pane runs: the user's login shell, or a given command with its
/// arguments, each in a given working directory (doc-9 R13).
///
/// Which of the two a pane was started with is stored here, in the variant
/// itself — never inferred from whether `argv` happens to be empty — because
/// [`ProgramStatus`]'s own stay-vs-close rule (R97) hinges on exactly this
/// fact once the program has ended, and a plain shell that is *given* as a
/// one-element `argv` would otherwise be indistinguishable from the kind
/// that must close its pane when it exits.
///
/// `cwd` is the directory the program was asked to start in. Restarting a
/// [`Program::Command`] (R97: "can run the same command again in the same
/// directory") always uses this stored value, never a directory the running
/// program may have `cd`'d to since — the two can and do diverge, and only
/// this one is what "the same directory" means.
///
/// Crosses the API as a pane's own field (doc-2 section 8: "program"), so it
/// is defined once, here (doc-16 R105), the same reasoning as
/// [`crate::Layout`]. Serde's default, externally-tagged representation is
/// used, not an internally-tagged one, the same reason `Layout` gives: an
/// internal tag needs every variant to serialize as a map, and neither
/// variant here is reducible to one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Program {
    /// The user's login shell.
    Shell {
        /// Its working directory.
        cwd: String,
    },
    /// A given command and its arguments.
    Command {
        /// The command and its arguments, in the order given on the command
        /// line: `argv[0]` is the program to run.
        argv: Vec<String>,
        /// Its working directory.
        cwd: String,
    },
}

impl Program {
    /// This program's working directory, regardless of which kind it is.
    #[must_use]
    pub fn cwd(&self) -> &str {
        match self {
            Program::Shell { cwd } | Program::Command { cwd, .. } => cwd,
        }
    }
}

/// What steg knows about a pane's program right now, in the first release
/// (doc-15 R97): running, or ended with an exit code. A richer `Status` —
/// waiting, failed, the bell, finished, with its own timers (doc-11) — is
/// built on top of this in a later task; this is the one fact the kernel
/// itself can always know with no clock of its own.
///
/// Crosses the API as a pane's own field (doc-2 section 8: "status" and
/// "exit_code"), so it is defined once, here (doc-16 R105).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramStatus {
    /// The program is running.
    Running,
    /// The program ended with this exit code.
    Ended {
        /// The exit code the program ended with.
        exit_code: i32,
    },
}

#[cfg(test)]
mod tests {
    use super::{Program, ProgramStatus};

    #[test]
    fn program_cwd_reads_either_variant() {
        let shell = Program::Shell {
            cwd: "/tmp".to_string(),
        };
        let command = Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        };

        assert_eq!(shell.cwd(), "/tmp");
        assert_eq!(command.cwd(), "/tmp");
    }

    #[test]
    fn shell_serde_round_trips() {
        let program = Program::Shell {
            cwd: "/home/bruk".to_string(),
        };

        let json = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<Program>(&json).unwrap(), program);
    }

    #[test]
    fn shell_serialises_as_an_object_keyed_shell() {
        let json = serde_json::to_value(Program::Shell {
            cwd: "/home/bruk".to_string(),
        })
        .unwrap();

        assert!(json.get("shell").is_some(), "{json}");
    }

    #[test]
    fn command_serde_round_trips_with_several_arguments() {
        let program = Program::Command {
            argv: vec!["htop".to_string(), "-d".to_string(), "5".to_string()],
            cwd: "/tmp/project".to_string(),
        };

        let json = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<Program>(&json).unwrap(), program);
    }

    #[test]
    fn command_serialises_as_an_object_keyed_command() {
        let json = serde_json::to_value(Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        })
        .unwrap();

        assert!(json.get("command").is_some(), "{json}");
    }

    #[test]
    fn running_serde_round_trips_through_its_own_word() {
        let json = serde_json::to_string(&ProgramStatus::Running).unwrap();
        assert_eq!(json, "\"running\"");
        assert_eq!(
            serde_json::from_str::<ProgramStatus>(&json).unwrap(),
            ProgramStatus::Running
        );
    }

    #[test]
    fn ended_serde_round_trips_with_its_exit_code() {
        let status = ProgramStatus::Ended { exit_code: 3 };

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(
            serde_json::from_str::<ProgramStatus>(&json).unwrap(),
            status
        );
    }

    #[test]
    fn ended_serialises_as_an_object_keyed_ended() {
        let json = serde_json::to_value(ProgramStatus::Ended { exit_code: 0 }).unwrap();

        assert!(json.get("ended").is_some(), "{json}");
    }

    #[test]
    fn a_negative_exit_code_round_trips() {
        // Not every platform's exit code fits in a u8 (a process killed by
        // a signal, on some reporting conventions, or a value this layer
        // never interprets) - i32 is deliberately wider than that, and the
        // wire form must carry a negative value without losing it.
        let status = ProgramStatus::Ended { exit_code: -1 };

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(
            serde_json::from_str::<ProgramStatus>(&json).unwrap(),
            status
        );
    }
}
