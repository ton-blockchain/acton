use super::{TestReport, TestReporter, TestStatus};
use acton_config::color::OwoColorize;
use std::io::{Write, stdout};

pub(crate) struct DotReporter {
    tests: Vec<TestReport>,
}

impl DotReporter {
    pub(crate) const fn new() -> Self {
        Self { tests: Vec::new() }
    }

    fn print_status_dot(&self, status: &TestStatus) {
        print!("{}", self.get_colored_status_char(status));
        stdout().flush().ok();
    }

    const fn get_status_char(&self, status: &TestStatus) -> char {
        match status {
            TestStatus::Passed => '·',
            TestStatus::Failed => 'x',
            TestStatus::Skipped => '○',
            TestStatus::Todo => '□',
        }
    }

    fn get_colored_status_char(&self, status: &TestStatus) -> String {
        let char = self.get_status_char(status);
        match status {
            TestStatus::Passed => char.to_string().green().to_string(),
            TestStatus::Failed => char.to_string().red().to_string(),
            TestStatus::Skipped => char.to_string().yellow().to_string(),
            TestStatus::Todo => char.to_string().purple().to_string(),
        }
    }

    fn print_test_output(&self, test: &TestReport, output_type: &str) -> anyhow::Result<()> {
        let Some(execution) = &test.execution else {
            return Ok(());
        };

        let (content, label) = match output_type {
            "stdout" => {
                if execution.stdout.trim().is_empty() {
                    return Ok(());
                }
                (&execution.stdout, "stdout")
            }
            "stderr" => {
                if execution.stderr.trim().is_empty() {
                    return Ok(());
                }
                (&execution.stderr, "stderr")
            }
            _ => return Ok(()),
        };

        println!();
        println!(
            "{}",
            format!("{} | {} > {}", label, test.file_path.display(), test.name).dimmed()
        );

        let lines: Vec<&str> = content.trim().lines().collect();
        for line in lines {
            println!("{line}");
        }

        Ok(())
    }

    fn print_all_test_outputs(&self) -> anyhow::Result<()> {
        for test in &self.tests {
            if test.status == TestStatus::Failed {
                if let Some(message) = &test.message {
                    println!();
                    println!("{} {}: {}", "FAIL".red().bold(), test.name, message);
                }

                if let Some(location) = &test.location {
                    println!("  {}", location.format().dimmed());
                }
            }

            self.print_test_output(test, "stdout")?;
            self.print_test_output(test, "stderr")?;
        }

        Ok(())
    }
}

impl TestReporter for DotReporter {
    fn on_test_started(&mut self, _test: &TestReport) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        self.print_status_dot(&test.status);
        self.tests.push(test.clone());
        Ok(())
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        println!();
        self.print_all_test_outputs()?;
        Ok(())
    }
}
