{{#options}}

{{#option "_contract-name_" }}
Build only the specified contract and its transitive dependencies.

The value must match a `[contracts.<name>]` key in `Acton.toml`.
{{/option}}

{{#option "`--clear-cache`" }}
Clear the project compilation cache before building.
{{/option}}

{{#option "`--graph` _path_" }}
Write the dependency graph for the requested build set as a DOT file.
{{/option}}

{{#option "`--out-dir` _dir_" }}
Directory for generated JSON build artifacts.

Defaults to `[build].out-dir` when configured, otherwise `build/`.
{{/option}}

{{#option "`--gen-dir` _dir_" }}
Directory for generated dependency helper files such as
`gen/<dependency>_code.tolk`.

Defaults to `[build].gen-dir` when configured, otherwise `gen/`.
{{/option}}

{{#option "`--output-fift` _dir_" }}
Directory for compiled Fift output for `.tolk` contracts.

Defaults to `[build].output-fift` when configured. If neither the flag nor the
config value is set, Acton does not write `.fif` files.
{{/option}}

{{#option "`--info`" }}
Print compiled code and hash information for each successfully built contract.
{{/option}}

{{/options}}
