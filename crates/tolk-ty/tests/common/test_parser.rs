use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub properties: HashMap<String, String>,
    pub properties_order: Vec<String>,
    pub input: String,
    pub expected: String,
    pub files: HashMap<String, String>,
}

pub enum ParserState {
    WaitingForTestStart,
    ReadingProperties,
    ReadingName,
    ReadingInput,
    ReadingFiles,
    ReadingExpected,
}

pub struct TestParser;

impl TestParser {
    #[must_use]
    pub fn parse_all(content: &str) -> Vec<TestCase> {
        let mut tests = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let separator = "========================================================================";
        let thin_separator =
            "------------------------------------------------------------------------";

        let mut state = ParserState::WaitingForTestStart;
        let mut current_name = String::new();
        let mut current_properties = HashMap::new();
        let mut current_properties_order = Vec::new();
        let mut current_input = String::new();
        let mut current_expected = String::new();
        let mut current_files = HashMap::new();
        let mut current_content = String::new();
        let mut current_file_path = String::new();

        for line in lines {
            let line_trimmed = line.trim_end();

            match state {
                ParserState::WaitingForTestStart => {
                    if line_trimmed == separator {
                        state = ParserState::ReadingProperties;
                        current_properties = HashMap::new();
                        current_properties_order = Vec::new();
                        current_files = HashMap::new();
                    }
                }
                ParserState::ReadingProperties => {
                    if let Some(property_line) = line_trimmed.strip_prefix('@') {
                        if let Some(space_index) = property_line.find(' ') {
                            let key = property_line[..space_index].to_string();
                            let value = property_line[space_index + 1..].trim().to_string();
                            current_properties.insert(key.clone(), value);
                            current_properties_order.push(key);
                        } else {
                            // Support for flags like @only
                            let key = property_line.trim().to_string();
                            current_properties.insert(key.clone(), String::new());
                            current_properties_order.push(key);
                        }
                    } else if !line_trimmed.is_empty() {
                        current_name = line_trimmed.to_string();
                        state = ParserState::ReadingName;
                    }
                }
                ParserState::ReadingName => {
                    if line_trimmed == separator {
                        state = ParserState::ReadingInput;
                        current_content = String::new();
                    }
                }
                ParserState::ReadingInput => {
                    if line_trimmed == thin_separator {
                        current_input = current_content.trim().to_string();
                        state = ParserState::ReadingExpected;
                        current_content = String::new();
                    } else if let Some(stripped) = line_trimmed.strip_prefix("---FILE:") {
                        current_input = current_content.trim().to_string();
                        state = ParserState::ReadingFiles;
                        current_file_path = stripped.trim().to_string();
                        current_content = String::new();
                    } else {
                        current_content.push_str(line);
                        current_content.push('\n');
                    }
                }
                ParserState::ReadingFiles => {
                    if line_trimmed == separator {
                        if !current_file_path.is_empty() {
                            current_files.insert(
                                current_file_path.clone(),
                                current_content.trim().to_string(),
                            );
                        }
                        tests.push(TestCase {
                            name: current_name.clone(),
                            properties: current_properties.clone(),
                            properties_order: current_properties_order.clone(),
                            input: current_input.clone(),
                            expected: String::new(),
                            files: current_files.clone(),
                        });
                        state = ParserState::ReadingProperties;
                        current_properties = HashMap::new();
                        current_properties_order = Vec::new();
                        current_files = HashMap::new();
                        current_content = String::new();
                        current_file_path = String::new();
                    } else if line_trimmed == thin_separator {
                        if !current_file_path.is_empty() {
                            current_files.insert(
                                current_file_path.clone(),
                                current_content.trim().to_string(),
                            );
                        }
                        state = ParserState::ReadingExpected;
                        current_content = String::new();
                        current_file_path = String::new();
                    } else if let Some(stripped) = line_trimmed.strip_prefix("---FILE:") {
                        if !current_file_path.is_empty() {
                            current_files.insert(
                                current_file_path.clone(),
                                current_content.trim().to_string(),
                            );
                        }
                        current_file_path = stripped.trim().to_string();
                        current_content = String::new();
                    } else {
                        current_content.push_str(line);
                        current_content.push('\n');
                    }
                }
                ParserState::ReadingExpected => {
                    if line_trimmed == separator {
                        current_expected = current_content.trim().to_string();
                        tests.push(TestCase {
                            name: current_name.clone(),
                            properties: current_properties.clone(),
                            properties_order: current_properties_order.clone(),
                            input: current_input.clone(),
                            expected: current_expected.clone(),
                            files: current_files.clone(),
                        });
                        state = ParserState::ReadingProperties;
                        current_properties = HashMap::new();
                        current_properties_order = Vec::new();
                        current_files = HashMap::new();
                        current_content = String::new();
                    } else {
                        current_content.push_str(line);
                        current_content.push('\n');
                    }
                }
            }
        }

        if !current_name.is_empty() {
            if matches!(state, ParserState::ReadingExpected) {
                current_expected = current_content.trim().to_string();
            } else if matches!(state, ParserState::ReadingFiles) && !current_file_path.is_empty() {
                current_files.insert(current_file_path, current_content.trim().to_string());
            }
            tests.push(TestCase {
                name: current_name,
                properties: current_properties,
                properties_order: current_properties_order,
                input: current_input,
                expected: current_expected,
                files: current_files,
            });
        }

        tests
    }

    pub fn update_expected_batch(file_path: &Path, updates: Vec<(&str, &str)>) {
        let content = std::fs::read_to_string(file_path).expect("Failed to read test file");
        let tests = Self::parse_all(&content);
        let mut new_content = Vec::new();

        let separator = "========================================================================";
        let thin_separator =
            "------------------------------------------------------------------------";

        for test in tests {
            if !new_content.is_empty() {
                new_content.push(String::new());
            }

            new_content.push(separator.to_string());

            for key in &test.properties_order {
                if let Some(value) = test.properties.get(key) {
                    if value.is_empty() {
                        new_content.push(format!("@{key}"));
                    } else {
                        new_content.push(format!("@{key} {value}"));
                    }
                }
            }

            new_content.push(test.name.clone());
            new_content.push(separator.to_string());
            new_content.push(test.input.clone());

            for (file_path, file_content) in &test.files {
                new_content.push(format!("---FILE:{file_path}"));
                new_content.push(file_content.clone());
            }

            new_content.push(thin_separator.to_string());

            let expected_content = updates
                .iter()
                .find(|(name, _)| *name == test.name)
                .map(|(_, actual)| actual.to_string())
                .unwrap_or(test.expected);

            new_content.push(expected_content);
        }

        std::fs::write(file_path, new_content.join("\n") + "\n")
            .expect("Failed to write test file");
    }
}
