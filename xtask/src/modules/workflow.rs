use anyhow::{Context, Result};
use owo_colors::OwoColorize;

pub(crate) struct Workflow<'a, C> {
    pub(crate) name: &'a str,
    pub(crate) steps: &'a [WorkflowStep<C>],
}

impl<C> Workflow<'_, C> {
    pub(crate) fn run(&self, context: &C) -> Result<()> {
        println!("{}: {}", "Running workflow".green().bold(), self.name);

        for step in self.steps {
            println!("{}: {}", "Running step".green().bold(), step.name);
            (step.run)(context).with_context(|| {
                format!("workflow `{}` failed at step `{}`", self.name, step.name)
            })?;
        }

        Ok(())
    }
}

pub(crate) struct WorkflowStep<C> {
    pub(crate) name: &'static str,
    pub(crate) run: fn(&C) -> Result<()>,
}
