use crate::commands::common::error_fmt;
use crate::config::ActonConfig;
use abi::{ContractAbi, TypeAbi};
use anyhow::anyhow;
use owo_colors::OwoColorize;
use std::fs;
use std::path::{Path, PathBuf};

pub fn test_gen_cmd(
    contract_id: &str,
    wrapper_output: Option<String>,
    test_output: Option<String>,
) -> anyhow::Result<()> {
    let project_root = find_project_root_from_current_dir().ok_or_else(|| {
        anyhow!(
            "Could not find Acton.toml in project root. Make sure you're in a project directory."
        )
    })?;

    let config = ActonConfig::load().map_err(|e| anyhow!("Failed to load Acton.toml: {}", e))?;

    let contract_config = config
        .get_contract(contract_id)
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, contract_id)))?;

    let contract_path = project_root.join(&contract_config.src);

    if !contract_path.exists() {
        return Err(anyhow!(
            "Contract file not found: {} (specified in Acton.toml as '{}')",
            contract_path.display(),
            contract_config.src
        ));
    }

    let content = fs::read_to_string(&contract_path)
        .map_err(|e| anyhow!("Failed to read contract file: {}", e))?;

    let abi = abi::contract_abi(&content, contract_path.to_str().unwrap());
    let handled_messages = abi::extract_handled_messages(&content, contract_path.to_str().unwrap());

    let file_stem = contract_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(contract_id);

    let contract_name = to_pascal_case(file_stem);
    let original_contract_name = contract_id;

    let (wrapper_path, test_path) = determine_output_paths(
        &project_root,
        wrapper_output,
        test_output,
        &contract_name,
        original_contract_name,
    );

    if let Some(parent) = wrapper_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let storage_file_path = abi
        .storage
        .as_ref()
        .and_then(|s| s.pos.as_ref())
        .map(|pos| PathBuf::from(&pos.uri));

    let types_in_same_file = check_types_in_same_file(&contract_path, &storage_file_path, &abi);

    if types_in_same_file {
        let types_file_path = create_types_file(&contract_path)?;
        print_types_warning(&contract_path, &types_file_path, &abi);

        let wrapper_code = generate_wrapper(
            &contract_name,
            &abi,
            &handled_messages,
            &contract_config.name,
            Some(&types_file_path),
            &wrapper_path,
            Some(&types_file_path),
            true,
        );

        fs::write(&wrapper_path, wrapper_code)
            .map_err(|e| anyhow!("Failed to write wrapper file: {}", e))?;

        let test_code = generate_test(
            &contract_name,
            &abi,
            &wrapper_path,
            &project_root,
            Some(&types_file_path),
        );
        fs::write(&test_path, test_code)
            .map_err(|e| anyhow!("Failed to write test file: {}", e))?;
    } else {
        let wrapper_code = generate_wrapper(
            &contract_name,
            &abi,
            &handled_messages,
            &contract_config.name,
            storage_file_path.as_ref(),
            &wrapper_path,
            None,
            false,
        );

        fs::write(&wrapper_path, wrapper_code)
            .map_err(|e| anyhow!("Failed to write wrapper file: {}", e))?;

        let test_code = generate_test(&contract_name, &abi, &wrapper_path, &project_root, None);
        fs::write(&test_path, test_code)
            .map_err(|e| anyhow!("Failed to write test file: {}", e))?;
    }

    let wrapper_relative = wrapper_path
        .strip_prefix(&project_root)
        .unwrap_or(&wrapper_path)
        .to_string_lossy();

    let test_relative = test_path
        .strip_prefix(&project_root)
        .unwrap_or(&test_path)
        .to_string_lossy();

    println!("   {} {}", "Generated".green().bold(), wrapper_relative);
    println!("   {} {}", "Generated".green().bold(), test_relative);

    Ok(())
}

fn check_types_in_same_file(
    contract_path: &Path,
    storage_file_path: &Option<PathBuf>,
    abi: &ContractAbi,
) -> bool {
    let contract_path_str = contract_path.to_string_lossy().to_string();

    let storage_in_same_file = if let Some(storage_path) = storage_file_path {
        storage_path.to_string_lossy() == contract_path_str
    } else {
        false
    };

    let messages_in_same_file = abi.messages.iter().any(|msg| {
        msg.pos
            .as_ref()
            .map(|pos| pos.uri == contract_path_str)
            .unwrap_or(false)
    });

    storage_in_same_file || messages_in_same_file
}

fn create_types_file(contract_path: &Path) -> anyhow::Result<PathBuf> {
    let contract_dir = contract_path
        .parent()
        .ok_or_else(|| anyhow!("Failed to get contract directory"))?;
    let types_file_path = contract_dir.join("types.tolk");

    if !types_file_path.exists() {
        let types_content =
            "// Auto-generated types file\n// Move your Storage struct and message types here\n\n";
        fs::write(&types_file_path, types_content)
            .map_err(|e| anyhow!("Failed to create types.tolk: {}", e))?;
    }

    Ok(types_file_path)
}

fn print_types_warning(contract_path: &Path, types_file_path: &Path, abi: &ContractAbi) {
    println!("\n{}", "WARNING".yellow().bold());
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════".yellow()
    );
    println!();
    println!("Your contract defines types in the same file as the contract logic.");
    println!("Tests and wrappers cannot import from contract files directly.");
    println!();
    println!(
        "{} Please move the following types to {}:",
        "→".yellow().bold(),
        types_file_path.display()
    );
    println!();

    if let Some(storage) = &abi.storage
        && let Some(pos) = &storage.pos
        && pos.uri == contract_path.to_string_lossy()
    {
        println!("  • {} struct", "Storage".cyan().bold());
    }

    for message in &abi.messages {
        if let Some(pos) = &message.pos
            && pos.uri == contract_path.to_string_lossy()
        {
            println!("  • {} struct", message.name.cyan().bold());
        }
    }

    println!();
    println!("After moving the types, update your contract to import them:");
    println!("  {}", "import \"types\"".to_owned().dimmed());
    println!();
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════".yellow()
    );
    println!();
}

fn find_project_root_from_current_dir() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        let acton_toml = current.join("Acton.toml");
        if acton_toml.exists() {
            return Some(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_uppercase().next().unwrap_or(ch));
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    if result.is_empty() {
        result.push_str(s);
    }

    result
}

fn determine_output_paths(
    project_root: &Path,
    wrapper_output: Option<String>,
    test_output: Option<String>,
    contract_name: &str,
    original_contract_name: &str,
) -> (PathBuf, PathBuf) {
    let default_wrapper = project_root
        .join("tests")
        .join("wrappers")
        .join(format!("{}.tolk", contract_name));

    let default_test = project_root
        .join("tests")
        .join(format!("{}_test.tolk", original_contract_name));

    let wrapper_path = wrapper_output.map(PathBuf::from).unwrap_or(default_wrapper);

    let test_path = test_output.map(PathBuf::from).unwrap_or(default_test);

    (wrapper_path, test_path)
}

#[allow(clippy::too_many_arguments)]
fn generate_wrapper(
    contract_name: &str,
    abi: &ContractAbi,
    handled_messages: &[String],
    contract_build_name: &str,
    storage_file_path: Option<&PathBuf>,
    wrapper_path: &Path,
    types_file_path: Option<&PathBuf>,
    needs_types_file: bool,
) -> String {
    let mut code = String::new();

    code.push_str("import \"@stdlib/gas-payments\"\n");
    code.push_str("import \"../../.acton/build/build\"\n");
    code.push_str("import \"../../.acton/emulation/network\"\n");
    code.push_str("import \"../../.acton/testing/expect\"\n");
    code.push_str("import \"../../.acton/types/message\"\n");

    if needs_types_file {
        if let Some(types_path) = types_file_path {
            let types_import = get_relative_import_from_wrapper(wrapper_path, types_path);
            code.push_str(&format!(
                "import \"{}\"  // TODO: Move Storage and message types here\n",
                types_import
            ));
        }
    } else if let Some(storage_path) = storage_file_path {
        let storage_import = get_relative_import_from_wrapper(wrapper_path, storage_path);
        code.push_str(&format!("import \"{}\"\n", storage_import));
    }

    code.push('\n');

    code.push_str("struct SendMessageConfig {\n");
    code.push_str("    value: coins = ton(\"0.1\")\n");
    code.push_str("    bounce: bool = false\n");
    code.push_str("}\n\n");

    code.push_str(&format!("struct {} {{\n", contract_name));
    code.push_str("    address: address\n");
    code.push_str("    init: ContractState\n");
    code.push_str("}\n\n");

    if let Some(storage) = &abi.storage {
        code.push_str(&generate_from_storage(
            contract_name,
            storage,
            contract_build_name,
        ));
        code.push('\n');
    }

    for message_name in handled_messages {
        if let Some(message_type) = abi.messages.iter().find(|m| &m.name == message_name) {
            code.push_str(&generate_send_method(contract_name, message_type));
            code.push('\n');
        }
    }

    for get_method in &abi.get_methods {
        code.push_str(&generate_get_method(contract_name, get_method));
        code.push('\n');
    }

    code
}

fn generate_from_storage(
    contract_name: &str,
    _storage: &TypeAbi,
    contract_build_name: &str,
) -> String {
    let mut code = String::new();

    code.push_str(&format!(
        "fun {}.fromStorage(storage: Storage) {{\n",
        contract_name
    ));
    code.push_str("    val init = ContractState {\n");
    code.push_str(&format!(
        "        code: build(\"{}\"),\n",
        contract_build_name
    ));
    code.push_str("        data: storage.toCell(),\n");
    code.push_str("    };\n");
    code.push_str("    val address = AutoDeployAddress { stateInit: init }.calculateAddress();\n");
    code.push_str(&format!(
        "    return {} {{ address, init }}\n",
        contract_name
    ));
    code.push_str("}\n");

    code
}

fn generate_send_method(contract_name: &str, message_type: &TypeAbi) -> String {
    let mut code = String::new();
    let method_name = format!("send{}", message_type.name);

    let fields: Vec<_> = message_type
        .fields
        .iter()
        .filter(|f| f.name != "queryId")
        .collect();

    let params = fields
        .iter()
        .map(|f| format!("{}: {}", f.name, f.type_info.human_readable))
        .collect::<Vec<_>>()
        .join(", ");

    let params_str = if params.is_empty() {
        String::new()
    } else {
        format!("{}, ", params)
    };

    code.push_str(&format!(
        "fun {}.{}(self, from: address, {}config: SendMessageConfig = {{}}): SendResultList {{\n",
        contract_name, method_name, params_str
    ));
    code.push_str("    val msg = createMessage({\n");
    code.push_str("        bounce: config.bounce,\n");
    code.push_str("        value: config.value,\n");
    code.push_str("        dest: self.address,\n");

    if fields.is_empty() {
        code.push_str(&format!("        body: {} {{}},\n", message_type.name));
    } else {
        code.push_str(&format!("        body: {} {{\n", message_type.name));
        for field in &fields {
            code.push_str(&format!("            {},\n", field.name));
        }
        code.push_str("        },\n");
    }

    code.push_str("    });\n");
    code.push_str("    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)\n");
    code.push_str("}\n");

    code
}

fn generate_get_method(contract_name: &str, get_method: &abi::GetMethod) -> String {
    let mut code = String::new();
    let method_name = &get_method.name;

    let params = get_method
        .parameters
        .iter()
        .map(|p| format!("{}: {}", p.name, p.type_info.human_readable))
        .collect::<Vec<_>>()
        .join(", ");

    let return_type = &get_method.return_type.human_readable;

    if params.is_empty() {
        code.push_str(&format!(
            "fun {}.{}(self): {} {{\n",
            contract_name, method_name, return_type
        ));
        code.push_str(&format!(
            "    return net.runGetMethod(self.address, \"{}\")\n",
            method_name
        ));
    } else {
        code.push_str(&format!(
            "fun {}.{}(self, {}): {} {{\n",
            contract_name, method_name, params, return_type
        ));

        let args = get_method
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        code.push_str(&format!(
            "    return net.runGetMethod(self.address, \"{}\", {})\n",
            method_name, args
        ));
    }

    code.push_str("}\n");

    code
}

fn generate_test(
    contract_name: &str,
    abi: &ContractAbi,
    wrapper_path: &Path,
    project_root: &Path,
    types_file_override: Option<&PathBuf>,
) -> String {
    let mut code = String::new();

    code.push_str("import \"@stdlib/gas-payments\"\n");
    code.push_str("import \"../.acton/emulation/network\"\n");
    code.push_str("import \"../.acton/testing/expect\"\n");
    code.push_str("import \"../.acton/testing/transaction_expect\"\n");

    if let Some(types_path) = types_file_override {
        let types_import = get_relative_import_from_test_to_types(types_path, project_root);
        code.push_str(&format!("import \"{}\"\n", types_import));
    }

    let wrapper_import = get_relative_import_for_test(wrapper_path);
    code.push_str(&format!("import \"{}\"\n", wrapper_import));
    code.push('\n');

    code.push_str(&generate_example_test(contract_name));
    code.push('\n');

    code.push_str(&generate_setup_test(contract_name, abi));

    code
}

fn get_relative_import_from_test_to_types(types_path: &PathBuf, project_root: &Path) -> String {
    let test_dir = project_root.join("tests");

    let relative_path =
        pathdiff::diff_paths(types_path, test_dir).unwrap_or_else(|| types_path.to_path_buf());

    let import_path = relative_path.to_string_lossy().to_string();
    if import_path.ends_with(".tolk") {
        import_path[..import_path.len() - 5].to_string()
    } else {
        import_path
    }
}

fn get_relative_import_for_test(wrapper_path: &Path) -> String {
    let wrapper_name = wrapper_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("wrapper");

    format!("wrappers/{}", wrapper_name)
}

fn get_relative_import_from_wrapper(wrapper_path: &Path, storage_path: &Path) -> String {
    let wrapper_dir = wrapper_path.parent().unwrap_or_else(|| Path::new("."));

    if let Ok(relative) = storage_path.strip_prefix(wrapper_dir) {
        let import_path = relative.to_string_lossy().to_string();
        if import_path.ends_with(".tolk") {
            import_path[..import_path.len() - 5].to_string()
        } else {
            import_path
        }
    } else {
        let mut import_path = String::new();
        let mut current = wrapper_dir;

        while !storage_path.starts_with(current) {
            if let Some(parent) = current.parent() {
                import_path.push_str("../");
                current = parent;
            } else {
                break;
            }
        }

        if let Ok(relative) = storage_path.strip_prefix(current) {
            import_path.push_str(&relative.to_string_lossy());
        } else {
            import_path.push_str(&storage_path.to_string_lossy());
        }

        if import_path.ends_with(".tolk") {
            import_path[..import_path.len() - 5].to_string()
        } else {
            import_path
        }
    }
}

fn generate_setup_test(contract_name: &str, abi: &ContractAbi) -> String {
    let mut code = String::new();

    code.push_str("fun setupTest() {\n");

    if let Some(storage) = &abi.storage {
        code.push_str(&format!(
            "    val contract = {}.fromStorage({{",
            contract_name
        ));

        let storage_fields = storage
            .fields
            .iter()
            .map(|f| {
                let default_value = get_default_value(&f.type_info.human_readable);
                format!(" {}: {}", f.name, default_value)
            })
            .collect::<Vec<_>>()
            .join(",");

        code.push_str(&storage_fields);
        code.push_str(" });\n");
    } else {
        code.push_str(&format!(
            "    val contract = {}.fromStorage({{ }});\n",
            contract_name
        ));
    }

    code.push('\n');
    code.push_str("    val deployer = net.treasury(\"deployer\");\n");
    code.push_str("    val msg = createMessage({\n");
    code.push_str("        bounce: false,\n");
    code.push_str("        value: ton(\"1.0\"),\n");
    code.push_str("        dest: {\n");
    code.push_str("            stateInit: contract.init,\n");
    code.push_str("        },\n");
    code.push_str("    });\n");
    code.push('\n');
    code.push_str(
        "    val res = net.send(deployer.address, msg, SEND_MODE_PAY_FEES_SEPARATELY);\n",
    );
    code.push_str("    expect(res).toHaveSuccessfulDeploy({ to: contract.address });\n");
    code.push('\n');
    code.push_str("    val not_deployer = net.treasury(\"not_deployer\");\n");
    code.push('\n');
    code.push_str("    return (contract, deployer, not_deployer)\n");
    code.push_str("}\n");

    code
}

fn get_default_value(type_name: &str) -> &str {
    match type_name {
        "int" | "int64" | "int32" | "int16" | "int8" => "0",
        "uint" | "uint64" | "uint32" | "uint16" | "uint8" => "0",
        "coins" => "0",
        "bool" => "false",
        "address" => "address(\"EQD__________________________________________0vo\")",
        "cell" => "createEmptyCell()",
        "slice" => "createEmptySlice()",
        _ => "null",
    }
}

fn generate_example_test(_contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("get fun `test-basic-flow`() {\n");
    code.push_str("    val (contract, deployer, not_deployer) = setupTest();\n");
    code.push('\n');
    code.push_str("    // TODO: Add your test logic here\n");
    code.push_str("}\n");

    code
}
