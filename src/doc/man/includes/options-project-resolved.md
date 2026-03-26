{{#options}}

{{#option "`--manifest-path` _path_" }}
Path to the `Acton.toml` file to use for this invocation.

Use this when running the command outside the project directory or when the
manifest lives at a non-default location.
{{/option}}

{{#option "`--project-root` _path_" }}
Path to the project root to use for configuration discovery and relative
defaults.

This conflicts with `--manifest-path`.
{{/option}}

{{/options}}
