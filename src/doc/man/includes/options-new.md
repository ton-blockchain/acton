{{#options}}

{{#option "_path_" }}
Directory to create the project in. Use `.` to create a new project in the
current directory.
{{/option}}

{{#option "`--name` _name_" }}
Project name. If not provided, interactive mode prompts for it and defaults to
the target directory name.
{{/option}}

{{#option "`--description` _description_" }}
Project description written to `Acton.toml`. If not provided, interactive mode
prompts for it and defaults to `A TON blockchain project`.
{{/option}}

{{#option "`--template` _template_" }}
Project template to use.

Possible values: `empty`, `counter`, `jetton`

If not provided, interactive mode prompts for the template.
{{/option}}

{{#option "`--license` _license_" }}
License identifier to place into `Acton.toml` and use for the generated
`LICENSE` file when Acton has a built-in template for that license.

If not provided, interactive mode prompts for it.
{{/option}}

{{#option "`--app`" }}
Include the template's TypeScript app scaffold when available.

At the moment only the `counter` template supports the app layout. If you pass
`--app` for a template that does not support it, `acton new` fails.

If the selected template supports the app layout and `--app` is not provided,
interactive mode prompts for this choice. Non-interactive mode leaves the app
layout disabled unless `--app` is passed explicitly.
{{/option}}

{{#option "`--hooks`" }}
Create and install the default project-local Git hooks.

If `git` is available and `--hooks` is not provided, interactive mode prompts
for this choice. Non-interactive mode leaves hooks disabled unless `--hooks`
is passed explicitly.
{{/option}}

{{#option "`--agents`" }}
Include an `AGENTS.md` file with coding-agent guidance from the selected
template.

If `--agents` is not provided, interactive mode prompts for this choice.
Non-interactive mode leaves `AGENTS.md` disabled unless `--agents` is passed
explicitly.
{{/option}}

{{/options}}
