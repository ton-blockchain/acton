pub mod build;
pub(crate) mod common;
pub mod compile;
pub mod disasm;
pub mod init;
pub mod new;
pub mod script;
pub mod test;
pub mod test_gen;
pub mod verify;

use std::collections::HashMap;

pub trait Command {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, args: &[String]) -> Result<(), String>;
}

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, command: Box<dyn Command>) {
        self.commands.insert(command.name().to_string(), command);
    }

    pub fn execute(&self, name: &str, args: &[String]) -> Result<(), String> {
        if let Some(command) = self.commands.get(name) {
            command.execute(args)
        } else {
            Err(format!("Unknown command: {name}"))
        }
    }

    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        self.commands
            .iter()
            .map(|(name, cmd)| (name.as_str(), cmd.description()))
            .collect()
    }
}
