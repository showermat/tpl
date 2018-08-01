# tpl

If you want a standalone utility that will render a basic text template for you, your options are surpringly limited.  Web
templating engines abound, but they're generally not standalone and they make plain-text templating feel like a second-class
citizen.  You may be able to write a wrapper around one that disables its HTML escaping and acts as a standalone program, but it
feels like a hack.  You could use `m4`, which is ancient and weird.  You could use `cpp`, which imposes C semantics on your
substitutions.  You could use a less-popular program like GPP, which provides a lot of functionality but isn't much prettier than
`cpp`.  Really, a "logic-less" format like Mustache's feels like the way to go, but if you use it you're stuck putting `{{{}}}`
around all of your substitutions to avoid HTML escaping, as if two braces wasn't already enough.  Obviously, the best solution is to
write your own program.

`tpl` accepts an input template and writes the rendered result to standard output.  It uses syntax similar to Mustache's, but with
configurable delimiters.  It accepts template arguments as YAML, either in a block at the top of the template or in a separate file.
That's it -- no more, no less.

## YAML Values

To determine what substitutiions are done by the template, a tree of values is passed into the program as YAML in one of two ways.

  - If the first first four characters in the template file are `---\n`, then the file is scanned and everything until the first
    occurrence of `\n...\n` is parsed as YAML and provided as values.  The template that is substituted begins immediately after
    this final newline.

  - The `-f` flag can be used when invoking the program to read values from another YAML file instead.

  - If values are given both in the template and through the `-f` flag, the two value trees will be *merged*, with the external
    file's values overriding.  Sequences are merged by appending.

Currently, the only keys supported in YAML mappings are strings of lower- and upper-case letters, digits, hyphen, and underscore.
Any others will be inaccessible because trying to use them in a template will cause a parsing error.  Additionally, the top-level
element in the YAML document must be a mapping.

For template conditionals (below), the following are considered false: `false`, `null`, empty sequences, empty mappings, and
nonexistent paths.  All other values are considered true, including zero and the empty string.

The special key `_config` in the top-level map allows setting configuration options for the current run of the program.  It is a
mapping with the following keys:

  - `open` (string): The opening delimiter for tags (default `{{`)
  - `close` (string): The closing delimiter for tags (default `open` backwards with characters `([{<` flipped)
  - `ignore` (boolean): Whether to replace unresolvable tags with empty strings rather than erroring (default false)

## Template Format

The template syntax is similar to that of [Mustache](https://mustache.github.io/mustache.5.html).

  - A "path" is used to traverse the value tree.  It's a list of YAML keys, separated by periods.  This path is taken relative to
    the current "context", which is the root by default.  Unlike Mustache, `tpl` does not search enclosing contexts if it doesn't
    find a key in the current context.  Instead, the path can start with `.` to resolve starting from the root, or `&` can be used
    to go up one level.  To select specific items out of a sequence, an integer can be used as the path element.

  - `{{path}}` is a direct substitution.  It takes the value at the path and substitutes it into the output.  It fails if the path
    does not exist or is not a stringifiable type (like a sequence or mapping).

  - `{{#path}}` is a conditional substitution that continues until a matching end marker `{{/}}`.  If the path is a non-empty array
    or list, it outputs everything until the end marker once for each item in the list, each time changing the context to that item.
    Otherwise, if the path is considered true (see values section above), it changes the context to that item and outputs everything
    until the end marker once.  Otherwise, it skips all output until the end marker.

  - `{{^path}}` is an inverse conditional substitution.  It works like the conditional substitution, but inverts the condition.  For
    this reason, it will never output its contents more than once.

  - `{{/}}` ends a conditional substitution.  It must not contain any text after the slash.

  - `{{?}}` is a key substitution.  It prints the key of the current context.  An integer can be added to query the key that many
    levels up.  `{{?}}` is the same is `{{?0}}`.

  - `{{!...}}` Denotes a comment.  The ellipsis can be replaced by any text except `}}`, and the entire tag will be removed from the
    output.  For commenting out blocks of the template, the block comment `{{!--...--}}` can be used, which will comment everything
    until `--}}` occurs.

  - Lambdas and partials are not supported by `tpl`.

## Build and Run

If you have the Rust toolchain installed, you can clone this repository and run `cargo build --install` as usual.  If you don't have
Rust installed, you can download the x86_64 binary from releases on GitHub.

Run the program like:

    tpl my-template.tpl [-f my-values.yaml]

## Credits

All content in this repository is created solely by me and released under the terms of the [Apache License, version
2.0](https://www.apache.org/licenses/LICENSE-2.0).
