mod handler;
mod init;

use crate::term;
use crate::utils::{copy, dump};

use anyhow::{Context, Result};
use reedline::{DefaultPrompt, Reedline, Signal};
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub use self::handler::*;

pub const REPL_COMMANDS: [(&str, &str, bool); 12] = [
    (".info", "Print the information", false),
    (".set", "Modify the configuration temporarily", false),
    (".role", "Specifies the role the AI will play", false),
    (".clear role", "Clear the currently selected role", false),
    (".prompt", "Add prompt, aka create a temporary role", true),
    (".history", "Print the history", false),
    (".clear history", "Clear the history", false),
    (".clear screen", "Clear the screen", false),
    (".multiline", "Enter multiline editor mode", true),
    (".copy", "Copy last reply message", false),
    (".help", "Print this help message", false),
    (".exit", "Exit the REPL", false),
];

pub struct Repl {
    editor: Reedline,
    prompt: DefaultPrompt,
}

impl Repl {
    pub fn run(&mut self, handler: ReplCmdHandler) -> Result<()> {
        dump(
            format!("Welcome to aichat {}", env!("CARGO_PKG_VERSION")),
            1,
        );
        dump("Type \".help\" for more information.", 1);
        let mut current_ctrlc = false;
        let handler = Arc::new(handler);
        loop {
            let handler_ctrlc = handler.get_ctrlc();
            if handler_ctrlc.load(Ordering::SeqCst) {
                handler_ctrlc.store(false, Ordering::SeqCst);
                current_ctrlc = true
            }
            match self.editor.read_line(&self.prompt) {
                Ok(Signal::Success(line)) => {
                    current_ctrlc = false;
                    match self.handle_line(handler.clone(), line) {
                        Ok(quit) => {
                            if quit {
                                break;
                            }
                        }
                        Err(err) => {
                            let err = format!("{err:?}");
                            dump(err.trim(), 2);
                        }
                    }
                }
                Ok(Signal::CtrlC) => {
                    if !current_ctrlc {
                        current_ctrlc = true;
                        dump("(To exit, press Ctrl+C again or Ctrl+D or type .exit)", 2);
                    } else {
                        break;
                    }
                }
                Ok(Signal::CtrlD) => {
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_line(&mut self, handler: Arc<ReplCmdHandler>, line: String) -> Result<bool> {
        if line.starts_with('.') {
            let (cmd, args) = match line.split_once(' ') {
                Some((head, tail)) => (head, Some(tail.trim())),
                None => (line.as_str(), None),
            };
            match cmd {
                ".exit" => {
                    return Ok(true);
                }
                ".help" => {
                    dump_repl_help();
                }
                ".clear" => match args {
                    Some("screen") => term::clear_screen(0)?,
                    Some("history") => {
                        let history = Box::new(self.editor.history_mut());
                        history.clear().with_context(|| "Failed to clear history")?;
                        dump("", 1);
                    }
                    Some("role") => handler.handle(ReplCmd::ClearRole)?,
                    _ => dump_unknown_command(),
                },
                ".history" => {
                    self.editor.print_history()?;
                    dump("", 1);
                }
                ".role" => match args {
                    Some(name) => handler.handle(ReplCmd::SetRole(name.to_string()))?,
                    None => dump("Usage: .role <name>", 2),
                },
                ".info" => {
                    handler.handle(ReplCmd::Info)?;
                }
                ".multiline" => {
                    let mut text = args.unwrap_or_default().to_string();
                    if text.is_empty() {
                        dump("Usage: .multiline { <your multiline content> }", 2);
                    } else {
                        if text.starts_with('{') && text.ends_with('}') {
                            text = text[1..text.len() - 1].to_string()
                        }
                        handler.handle(ReplCmd::Submit(text))?;
                    }
                }
                ".copy" => {
                    let reply = handler.get_reply();
                    if reply.is_empty() {
                        dump("No reply messages that can be copied", 1)
                    } else {
                        copy(&reply)?;
                        dump("Copied", 1);
                    }
                }
                ".set" => {
                    handler.handle(ReplCmd::UpdateConfig(args.unwrap_or_default().to_string()))?
                }
                ".prompt" => {
                    let mut text = args.unwrap_or_default().to_string();
                    if text.is_empty() {
                        dump("Usage: .prompt { <your multiline content> }.", 2);
                    } else {
                        if text.starts_with('{') && text.ends_with('}') {
                            text = text[1..text.len() - 1].to_string()
                        }
                        handler.handle(ReplCmd::Prompt(text))?;
                    }
                }
                _ => dump_unknown_command(),
            }
        } else {
            handler.handle(ReplCmd::Submit(line))?;
        }

        Ok(false)
    }
}

fn dump_unknown_command() {
    dump("Unknown command. Type \".help\" for more information.", 2);
}

fn dump_repl_help() {
    let head = REPL_COMMANDS
        .iter()
        .map(|(name, desc, _)| format!("{name:<15} {desc}"))
        .collect::<Vec<String>>()
        .join("\n");
    dump(
        format!("{head}\n\nPress Ctrl+C to abort session, Ctrl+D to exit the REPL"),
        2,
    );
}