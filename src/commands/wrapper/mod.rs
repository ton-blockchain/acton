use crate::commands::common::error_fmt;
use acton_config::config::ActonConfig;
use anyhow::anyhow;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use ton_abi::{ContractAbi, TypeAbi};

struct WrapperModel {
    project_root: PathBuf,
    contract_id: String,
    contract_name: String,
    contract_path: PathBuf,
    abi: ContractAbi,
    handled_messages: Vec<String>,
    storage_path: Option<PathBuf>,
    message_paths: Vec<PathBuf>,
    wrapper_path: PathBuf,
    test_path: PathBuf,
    mappings: Option<BTreeMap<String, String>>,
}

fn build_model(
    contract_id: &str,
    wrapper_output: Option<String>,
    test_output: Option<String>,
    storage_struct_name: Option<String>,
) -> anyhow::Result<WrapperModel> {
    let project_root = find_project_root_from_current_dir().ok_or_else(|| {
        anyhow!(
            "Could not find Acton.toml in project root. Make sure you're in a project directory."
        )
    })?;

    let config = ActonConfig::load().map_err(|e| anyhow!("Failed to load Acton.toml: {e}"))?;

    let contract_config = config
        .get_contract(contract_id)
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, contract_id)))?;

    let contract_path = project_root.join(&contract_config.src);

    if !contract_path.exists() {
        anyhow::bail!(color_print::cformat!(
            "Contract file for <yellow>{contract_id}</> not found: <yellow>{}</> (specified in Acton.toml as <yellow>{}</>)",
            contract_path.display(),
            contract_config.src
        ));
    }

    let content = fs::read_to_string(&contract_path)
        .map_err(|e| anyhow!("Failed to read contract file: {e}"))?;

    let contract_path_str = contract_path.to_str().unwrap_or_default();
    let mut abi = ton_abi::contract_abi(&content, contract_path_str, &config.mappings);
    let handled_messages =
        ton_abi::extract_handled_messages(&content, contract_path_str, &config.mappings);

    let file_stem = contract_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(contract_id);

    let contract_name = to_pascal_case(file_stem);

    if let Some(storage_name) = storage_struct_name {
        let storage = abi.types.iter().find(|t| t.name == storage_name).cloned();
        if let Some(storage) = storage {
            abi.storage = Some(storage);
        } else {
            anyhow::bail!(
                "Storage struct {} not found in contract {}. Available types:\n{}",
                storage_name.yellow(),
                contract_id.yellow(),
                abi.storages()
                    .iter()
                    .map(|t| format!(" {}", t.name.as_str().yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    } else if abi.storage.is_none() {
        let candidates = abi.storages();

        if candidates.len() == 1 {
            abi.storage = Some(candidates[0].clone());
        } else if !abi.storages().is_empty() {
            let options = abi
                .storages()
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>();
            let selection = inquire::Select::new("Select storage struct:", options).prompt()?;
            abi.storage = abi.types.iter().find(|t| t.name == selection).cloned();
        }
    }

    let storage_path = abi.storage.as_ref().map(|typ| PathBuf::from(&typ.pos.uri));
    let message_paths = abi
        .messages
        .iter()
        .map(|typ| typ.pos.uri.clone())
        .collect::<HashSet<_>>();

    let default_wrapper = project_root
        .join("tests")
        .join("wrappers")
        .join(format!("{contract_name}.tolk"));

    let default_test = project_root
        .join("tests")
        .join(format!("{contract_id}.test.tolk"));

    let wrapper_path = wrapper_output.map_or(default_wrapper, PathBuf::from);
    let test_path = test_output.map_or(default_test, PathBuf::from);

    let mut message_paths: Vec<PathBuf> = message_paths.iter().map(PathBuf::from).collect();
    message_paths.sort();

    Ok(WrapperModel {
        project_root,
        contract_id: contract_id.to_owned(),
        contract_name,
        contract_path,
        abi,
        handled_messages,
        storage_path,
        message_paths,
        wrapper_path,
        test_path,
        mappings: config.mappings.clone(),
    })
}

pub fn wrapper_cmd(
    contract_id: &str,
    wrapper_output: Option<String>,
    test_output: Option<String>,
    generate_test_stub: bool,
    storage_struct_name: Option<String>,
) -> anyhow::Result<()> {
    let model = build_model(
        contract_id,
        wrapper_output,
        test_output,
        storage_struct_name,
    )?;

    if let Some(parent) = model.wrapper_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create directory {}: {}", parent.display(), e))?;
    }
    if let Some(parent) = model.test_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let types_in_contract_file = is_types_in_contract_file(&model);

    let (wrapper_code, test_code) = if types_in_contract_file {
        let types_file_path = create_types_file(&model.contract_path)?;
        print_types_warning(&model.contract_path, &types_file_path, &model.abi);

        let wrapper_code = generate_wrapper(&model, Some(&types_file_path));
        let test_code = generate_test(&model, Some(&types_file_path));
        (wrapper_code, test_code)
    } else {
        let wrapper_code = generate_wrapper(&model, None);
        let test_code = generate_test(&model, None);
        (wrapper_code, test_code)
    };

    fs::write(&model.wrapper_path, wrapper_code)
        .map_err(|e| anyhow!("Failed to write wrapper file: {e}"))?;

    if generate_test_stub {
        fs::write(&model.test_path, test_code)
            .map_err(|e| anyhow!("Failed to write test file: {e}"))?;
    }

    let wrapper_relative = model
        .wrapper_path
        .strip_prefix(&model.project_root)
        .unwrap_or(&model.wrapper_path)
        .to_string_lossy();

    let test_relative = model
        .test_path
        .strip_prefix(&model.project_root)
        .unwrap_or(&model.test_path)
        .to_string_lossy();

    println!("   {} {}", "Generated".green().bold(), wrapper_relative);

    if generate_test_stub {
        println!("   {} {}", "Generated".green().bold(), test_relative);
    }

    Ok(())
}

fn is_types_in_contract_file(model: &WrapperModel) -> bool {
    let storage_in_contract_file =
        matches!(&model.storage_path, Some(storage_path) if storage_path == &model.contract_path);

    let messages_in_contract_file = model
        .message_paths
        .iter()
        .any(|msg| msg == &model.contract_path);

    storage_in_contract_file || messages_in_contract_file
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
            .map_err(|e| anyhow!("Failed to create types.tolk: {e}"))?;
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
        types_file_path.display().green()
    );
    println!();

    if let Some(storage) = &abi.storage
        && storage.pos.uri == contract_path.to_string_lossy()
    {
        println!("  • {} struct", "Storage".cyan().bold());
    }

    for message in &abi.messages {
        if message.pos.uri == contract_path.to_string_lossy() {
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

fn generate_wrapper(model: &WrapperModel, types_file_path: Option<&PathBuf>) -> String {
    let proot = &model.project_root;
    let root = &model.wrapper_path;
    let contract = &model.contract_name;
    let mappings = &model.mappings;

    let mut code = String::new();

    code.push_str("import \"@stdlib/gas-payments\"\n");
    code.push_str(&import_stdlib("build/build"));
    code.push_str(&import_stdlib("emulation/network"));
    code.push_str(&import_stdlib("testing/assert"));
    code.push_str(&import_stdlib("testing/expect"));
    code.push_str(&import_stdlib("types/message"));

    if let Some(types_path) = types_file_path {
        let types_import = get_import_path(proot, root, types_path, mappings);
        code.push_str(&gen_import_path(types_import));
    }

    if let Some(storage_path) = &model.storage_path
        && Some(storage_path) != types_file_path
    {
        // add storage file import only if it different from types file
        let storage_import = get_import_path(proot, root, storage_path, mappings);
        code.push_str(&gen_import_path(storage_import));
    }

    for messages_path in &model.message_paths {
        if Some(messages_path) == types_file_path
            || Some(messages_path) == model.storage_path.as_ref()
        {
            // don't add duplicate import
            continue;
        }

        if messages_path == &model.contract_path {
            // never import file with contract itself since this will break all
            continue;
        }

        let types_import = get_import_path(proot, root, messages_path, mappings);
        code.push_str(&gen_import_path(types_import));
    }

    code.push('\n');

    code.push_str(&format!("struct {contract} {{\n"));
    code.push_str("    address: address\n");
    code.push_str("    stateInit: ContractState? = null\n");
    code.push_str("}\n\n");

    if model.abi.storage.is_some() {
        code.push_str(&generate_from_storage(
            contract,
            &model.contract_id,
            model
                .abi
                .storage
                .as_ref()
                .map(|s| s.name.clone())
                .as_deref()
                .unwrap_or("Storage"),
        ));
    } else {
        code.push_str(&generate_empty_from_storage(contract, &model.contract_id));
    }

    code.push('\n');
    code.push_str(&generate_from_address(contract));
    code.push('\n');
    code.push_str(&generate_deploy(contract));
    code.push('\n');

    for message_name in &model.handled_messages {
        if let Some(message_type) = model.abi.messages.iter().find(|m| &m.name == message_name) {
            code.push_str(&generate_send_method(contract, message_type));
            code.push('\n');
        }
    }

    code.push_str(&generate_send_any_method(contract));
    code.push('\n');

    for get_method in &model.abi.get_methods {
        code.push_str(&generate_get_method(contract, get_method));
        code.push('\n');
    }

    format!("{}\n", code.trim())
}

fn generate_from_storage(
    contract_name: &str,
    contract_build_name: &str,
    storage_name: &str,
) -> String {
    let mut code = String::new();

    code.push_str("/// Creates a contract wrapper instance from the storage data\n");
    code.push_str(&format!(
        "fun {contract_name}.fromStorage(storage: {storage_name}, toShard: AddressShardingOptions? = null) {{\n",
    ));
    code.push_str("    val stateInit = ContractState {\n");
    code.push_str(&format!(
        "        code: build(\"{contract_build_name}\"),\n",
    ));
    code.push_str("        data: storage.toCell(),\n");
    code.push_str("    };\n");
    code.push_str("    val address = toShard == null\n");
    code.push_str("        ? AutoDeployAddress { stateInit }.calculateAddress()\n");
    code.push_str("        : AutoDeployAddress { stateInit, toShard }.calculateAddress();\n");
    code.push_str(&format!(
        "    return {contract_name} {{ address, stateInit }}\n",
    ));
    code.push_str("}\n");

    code
}

fn generate_from_address(contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Creates a contract wrapper instance from the address\n");
    code.push_str(&format!(
        "fun {contract_name}.fromAddress(address: address) {{\n"
    ));
    code.push_str(&format!("    return {contract_name} {{ address }}\n",));
    code.push_str("}\n");

    code
}

fn generate_empty_from_storage(contract_name: &str, contract_build_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Creates a contract wrapper instance from the storage data\n");
    code.push_str(&format!(
        "fun {contract_name}.fromStorage(toShard: AddressShardingOptions? = null) {{\n"
    ));
    code.push_str("    val stateInit = ContractState {\n");
    code.push_str(&format!(
        "        code: build(\"{contract_build_name}\"),\n"
    ));
    code.push_str("        data: createEmptyCell(),\n");
    code.push_str("    };\n");
    code.push_str("    val address = toShard == null\n");
    code.push_str("        ? AutoDeployAddress { stateInit }.calculateAddress()\n");
    code.push_str("        : AutoDeployAddress { stateInit, toShard }.calculateAddress();\n");
    code.push_str(&format!(
        "    return {contract_name} {{ address, stateInit }}\n"
    ));
    code.push_str("}\n");

    code
}

fn generate_deploy(contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Deploys the contract to the blockchain\n");
    code.push_str(&format!(
        "fun {contract_name}.deploy(self, from: address, config: SendParams = {{}}): SendResultList {{\n",
    ));
    code.push_str("    if (self.stateInit == null) {\n");
    code.push_str("        Assert.fail(\"Cannot deploy a contract created with 'fromAddress' because it lacks state init for deployment\");\n");
    code.push_str("    }\n");
    code.push_str("    val msg = createMessage({\n");
    code.push_str("        bounce: config.bounce,\n");
    code.push_str("        value: config.value,\n");
    code.push_str("        dest: {\n");
    code.push_str("            stateInit: self.stateInit,\n");
    code.push_str("        },\n");
    code.push_str("    });\n");
    code.push_str("    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)\n");
    code.push_str("}\n");

    code
}

fn generate_send_method(contract_name: &str, message_type: &TypeAbi) -> String {
    let mut code = String::new();
    let method_name = format!("send{}", message_type.name);

    let fields: Vec<_> = message_type.fields.iter().collect();

    let params = fields
        .iter()
        .map(|f| {
            let type_name =
                if let ton_abi::BaseTypeInfo::Cell { inner: Some(inner) } = &f.type_info.base {
                    &inner.human_readable
                } else {
                    &f.type_info.human_readable
                };
            let name = normalize_param_name(&f.name);
            format!("{}: {}", name, type_name)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let params_str = if params.is_empty() {
        String::new()
    } else {
        format!("{params}, ")
    };

    code.push_str(&format!(
        "fun {contract_name}.{method_name}(self, from: address, {params_str}config: SendParams = {{}}): SendResultList {{\n",
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
            let param_name = normalize_param_name(&field.name);

            if let ton_abi::BaseTypeInfo::Cell { inner: Some(_) } = &field.type_info.base {
                code.push_str(&format!(
                    "            {}: {}.toCell(),\n",
                    field.name, param_name
                ));
            } else if field.name == param_name {
                code.push_str(&format!("            {},\n", field.name));
            } else {
                code.push_str(&format!("            {}: {},\n", field.name, param_name));
            }
        }
        code.push_str("        },\n");
    }

    code.push_str("    });\n");
    code.push_str("    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)\n");
    code.push_str("}\n");

    code
}

fn normalize_param_name(name: &str) -> String {
    if name == "from" || name == "config" {
        format!("{}_", name)
    } else {
        name.to_owned()
    }
}

fn generate_send_any_method(contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Send message to the contract with a custom body cell\n");
    code.push_str(&format!(
        "fun {contract_name}.sendAny(self, from: address, body: cell, config: SendParams = {{}}): SendResultList {{\n",
    ));
    code.push_str("    val msg = createMessage({\n");
    code.push_str("        bounce: config.bounce,\n");
    code.push_str("        value: config.value,\n");
    code.push_str("        dest: self.address,\n");
    code.push_str("        body,\n");
    code.push_str("    });\n");
    code.push_str("    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)\n");
    code.push_str("}\n");

    code
}

fn generate_get_method(contract_name: &str, get_method: &ton_abi::GetMethod) -> String {
    let mut code = String::new();
    let method_name = &get_method.name;

    let params = get_method
        .parameters
        .iter()
        .map(|p| {
            let type_name =
                if let ton_abi::BaseTypeInfo::Cell { inner: Some(inner) } = &p.type_info.base {
                    &inner.human_readable
                } else {
                    &p.type_info.human_readable
                };
            format!("{}: {}", p.name, type_name)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let return_type = &get_method.return_type.human_readable;

    if params.is_empty() {
        code.push_str(&format!(
            "fun {contract_name}.{method_name}(self): {return_type} {{\n"
        ));
        code.push_str(&format!(
            "    return net.runGetMethod(self.address, \"{method_name}\")\n"
        ));
    } else {
        code.push_str(&format!(
            "fun {contract_name}.{method_name}(self, {params}): {return_type} {{\n"
        ));

        let args = get_method
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>();

        if args.is_empty() {
            code.push_str(&format!(
                "    return net.runGetMethod(self.address, \"{method_name}\")\n"
            ));
        } else if args.len() == 1 {
            let arg_name = if let ton_abi::BaseTypeInfo::Cell { inner: Some(_) } =
                &get_method.parameters[0].type_info.base
            {
                format!("{}.toCell()", args[0])
            } else {
                args[0].to_string()
            };

            code.push_str(&format!(
                "    return net.runGetMethod(self.address, \"{method_name}\", {arg_name})\n"
            ));
        } else {
            let args = get_method
                .parameters
                .iter()
                .map(|p| {
                    if let ton_abi::BaseTypeInfo::Cell { inner: Some(_) } = &p.type_info.base {
                        format!("{}.toCell()", p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            code.push_str(&format!(
                "    return net.runGetMethod(self.address, \"{method_name}\", [{args}] as tuple)\n"
            ));
        }
    }

    code.push_str("}\n");

    code
}

fn generate_test(model: &WrapperModel, types_file_override: Option<&PathBuf>) -> String {
    let proot = &model.project_root;
    let root = &model.test_path;
    let contract = &model.contract_name;
    let mappings = &model.mappings;

    let mut code = String::new();

    code.push_str("import \"@stdlib/gas-payments\"\n");
    code.push_str(&import_stdlib("emulation/network"));
    code.push_str(&import_stdlib("testing/expect"));
    code.push_str(&import_stdlib("testing/transaction_expect"));

    if let Some(types_path) = types_file_override {
        let types_import = get_import_path(proot, root, types_path, mappings);
        code.push_str(&gen_import_path(types_import));
    }

    for messages_path in &model.message_paths {
        if Some(messages_path) == types_file_override {
            // don't add duplicate import
            continue;
        }

        if messages_path == &model.contract_path {
            // never import file with contract itself since this will break all
            continue;
        }

        let types_import = get_import_path(proot, root, messages_path, mappings);
        code.push_str(&gen_import_path(types_import));
    }

    let wrapper_import = get_import_path(proot, root, &model.wrapper_path, mappings);
    code.push_str(&gen_import_path(wrapper_import));
    code.push('\n');

    code.push_str(&generate_example_test(contract));
    code.push('\n');

    code.push_str(&generate_setup_test(contract, &model.abi));

    format!("{}\n", code.trim())
}

fn import_stdlib(path: &str) -> String {
    gen_import_path(PathBuf::from("@acton").join(path))
}

fn gen_import_path(path: PathBuf) -> String {
    let path = path.display().to_string();
    format!(
        "import \"{}\"\n",
        path.trim_start_matches("./").trim_end_matches(".tolk")
    )
}

fn get_relative_import(project_root: &Path, where_: &Path, what: &Path) -> PathBuf {
    let Some(where_dir) = where_.parent() else {
        return what.to_path_buf();
    };
    let what_abs_path = project_root.join(what);
    let where_abs_dir = project_root.join(where_dir);

    let relative_path = pathdiff::diff_paths(&what_abs_path, where_abs_dir);
    relative_path.unwrap_or(what_abs_path)
}

fn get_import_path(
    project_root: &Path,
    where_: &Path,
    what: &Path,
    mappings: &Option<BTreeMap<String, String>>,
) -> PathBuf {
    if let Some(mapped_import) = resolve_mapped_import(project_root, what, mappings) {
        return mapped_import;
    }

    get_relative_import(project_root, where_, what)
}

fn resolve_mapped_import(
    project_root: &Path,
    what: &Path,
    mappings: &Option<BTreeMap<String, String>>,
) -> Option<PathBuf> {
    let mappings = mappings.as_ref()?;
    let what_abs = normalize_abs_path(project_root, what);

    let mut best_match = None;

    for (key, value) in mappings {
        let mapping_abs = normalize_abs_path(project_root, Path::new(value));

        if !what_abs.starts_with(&mapping_abs) {
            continue;
        }

        let Ok(relative_path) = what_abs.strip_prefix(&mapping_abs) else {
            continue;
        };

        let key = if key.starts_with('@') {
            key.clone()
        } else {
            format!("@{key}")
        };

        let mut import_path = PathBuf::from(key);
        if !relative_path.as_os_str().is_empty() {
            import_path = import_path.join(relative_path);
        }

        let score = mapping_abs.components().count();
        if best_match
            .as_ref()
            .map(|(best_score, _)| score > *best_score)
            .unwrap_or(true)
        {
            best_match = Some((score, import_path));
        }
    }

    best_match.map(|(_, path)| path)
}

fn normalize_abs_path(project_root: &Path, path: &Path) -> PathBuf {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    abs_path.canonicalize().unwrap_or(abs_path)
}

fn generate_setup_test(contract_name: &str, abi: &ContractAbi) -> String {
    let mut code = String::new();

    code.push_str(
        "/// Initializes the test environment, creating a fresh instance of the contract.\n",
    );
    code.push_str("/// Returns the contract wrapper and two treasury accounts (`deployer` and `not_deployer`).\n");
    code.push_str("fun setupTest() {\n");

    code.push_str("    // Create a treasury account for deployment (typically the owner)\n");
    code.push_str("    val deployer = net.treasury(\"deployer\");\n");
    code.push_str(
        "    // Create another treasury account for testing interactions from other users\n",
    );
    code.push_str("    val not_deployer = net.treasury(\"not_deployer\");\n");
    code.push('\n');
    code.push_str("    // Initialize and deploy the contract with default values\n");

    if let Some(storage) = &abi.storage {
        code.push_str(&format!(
            "    val contract = {contract_name}.fromStorage({{"
        ));

        let storage_fields = storage
            .fields
            .iter()
            .map(|f| {
                let default_value = get_default_value(&f.type_info.human_readable);
                match &f.type_info.base {
                    ton_abi::BaseTypeInfo::Cell { inner: Some(inner) } => {
                        let default_value = get_default_value(&inner.human_readable);
                        format!(" {}: {}.toCell()", f.name, default_value)
                    }
                    _ => format!(" {}: {}", f.name, default_value),
                }
            })
            .collect::<Vec<_>>()
            .join(",");

        code.push_str(&storage_fields);
        code.push_str(" });\n");
    } else {
        code.push_str(&format!(
            "    val contract = {contract_name}.fromStorage();\n"
        ));
    }

    code.push_str("    val res = contract.deploy(deployer.address, { value: ton(\"1\") });\n");
    code.push_str("    expect(res).toHaveSuccessfulDeploy({ to: contract.address });\n");
    code.push('\n');
    code.push_str("    return (contract, deployer, not_deployer)\n");
    code.push_str("}\n");

    code
}

fn get_default_value(type_name: &str) -> &str {
    match type_name {
        _ if type_name.starts_with("int") => "0",
        _ if type_name.starts_with("uint") => "0",
        "coins" => "0",
        "bool" => "false",
        "address" => "address(\"EQD__________________________________________0vo\")",
        "any_address" => "address(\"EQD__________________________________________0vo\")",
        "cell" => "createEmptyCell()",
        "slice" => "createEmptySlice()",
        _ if type_name.starts_with("map<") => "createEmptyMap()",
        _ => "null",
    }
}

fn generate_example_test(_contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Example test case demonstrating the basic flow\n");
    code.push_str("get fun `test-basic-flow`() {\n");
    code.push_str("    val (contract, deployer, not_deployer) = setupTest();\n");
    code.push('\n');
    code.push_str("    // TODO: Implement your test logic here\n");
    code.push_str("    // Example:\n");
    code.push_str("    // val res = contract.sendMsg(deployer.address, ...);\n");
    code.push_str("    // expect(res).toHaveTransaction({ ... });\n");
    code.push_str("}\n");

    code
}
