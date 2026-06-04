![DTR banner](https://raw.githubusercontent.com/naginagiyev/dtr/main/assets/banner.png)

# DTR — Dynamic Text Renderer

**DTR** is a Python library built in Rust ([PyO3](https://pyo3.rs/)) for rendering template strings with custom function calls, file includes, and variables. Use it to generate dynamic text from templates while keeping logic in Python.

[![Python 3.8+](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/naginagiyev/dtr/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/rust-powered-orange.svg)](https://www.rust-lang.org/)

## Features

- **Function calls** — Register Python callables and invoke them from templates (`~fn:name(args)`).
- **File includes** — Pull in external template files (`~include:path`).
- **Variables** — Resolve `$variables` inside function arguments.
- **Custom syntax** — Optional prefixes and name patterns via `Syntax`.
- **Fast core** — Parsing and rendering run in Rust; Python is used for your functions.

## Installation

```bash
pip install dtrlib
```

### Install from source

Requires [Rust](https://rustup.rs/) and [maturin](https://www.maturin.rs/):

```bash
git clone https://github.com/naginagiyev/dtr.git
cd dtr
pip install maturin
maturin develop
```

## Quick start

```python
from dtr import Compiler

compiler = Compiler()

def repeat(args):
    text = str(args[0]) if args else ""
    count = int(args[1]) if len(args) > 1 else 1
    return text * count

compiler.add_function("str.repeat", repeat)

template = "Result: ~fn:str.repeat('ha', 3)"
print(compiler.compile(template))
# Result: hahaha
```

## Template syntax

Default syntax (configurable with `Syntax`):

| Feature | Syntax | Example |
|--------|--------|---------|
| Function call | `~fn:name(args)` | `~fn:math.add(1, 2)` |
| Include file | `~include:path` | `~include:"footer.txt"` |
| Variable (in function args) | `$name` | `~fn:greet($user)` |

### Function calls

Register a function with its full dotted name, then call it in text:

```python
compiler = Compiler()

def upper(args):
    return str(args[0]).upper() if args else ""

compiler.add_function("text.upper", upper)
print(compiler.compile("~fn:text.upper('hello')"))
# HELLO
```

Arguments support strings, numbers, booleans, lists, and dicts. Empty arguments become `null`.

### Register a Python file as a module

Load a `.py` file and expose its top-level `def` functions under a namespace:

```python
compiler = Compiler()

compiler.add_module("random_number_generator.py", "generator")
print(compiler.compile("~fn:generator.roll(6)"))
```

- **`module_path`**: path to the `.py` file (relative to the current working directory or absolute).
- **`as_name`**: namespace used in templates (`generator.func`, not the filename).

Only module-level functions are registered; names starting with `_` are skipped. Classes, variables, and other attributes are ignored. Re-registering the same namespace overwrites previous functions for matching names.

### Variables

Set values with `set_arg`, then reference them inside **function arguments**:

```python
compiler.set_arg("name", "Ada")
print(compiler.compile("~fn:text.upper($name)"))
# ADA
```

### File includes

Includes are resolved relative to the current file path (or the working directory when compiling from a string):

```python
output = compiler.compile_with_file(template, "letters/template.txt")
```

Circular includes are detected; behavior depends on `debug_mode` (see below).

### Custom syntax

```python
from dtr import Compiler, Syntax

syntax = Syntax(
    function_prefix="@@",
    include_prefix="@@include:",
    variable_prefix="@",
)
compiler = Compiler(syntax)
```

## API reference

### `Compiler`

| Method | Description |
|--------|-------------|
| `compile(text)` | Render a string (virtual file name `<input>`). |
| `compile_with_file(text, file_name)` | Render with a path used for includes and errors. |
| `set_debug_mode(enabled)` | When `True`, errors raise; when `False`, failed calls/includes are left unchanged. |
| `set_arg(name, value)` | Set a variable for use in function arguments. |
| `clear_args()` | Remove all variables. |
| `add_function(full_name, callable)` | Register a Python callable; receives a list of parsed argument values. |
| `add_module(module_path, as_name)` | Load a `.py` file and register its top-level functions as `as_name.function`. |

### `Syntax`

Optional constructor arguments: `function_prefix`, `include_prefix`, `variable_prefix`, `function_name_pattern`, `variable_name_pattern`.

Defaults:

- Function prefix: `~fn:`
- Include prefix: `~include:`
- Variable prefix: `$`

## Development

```bash
pip install maturin
maturin develop
maturin build --release
```

Run a quick check:

```python
from dtr import Compiler
assert Compiler().compile("plain text") == "plain text"
```

## Publishing

```bash
maturin build --release
maturin publish
```

Use `maturin publish --repository testpypi` to test on [TestPyPI](https://test.pypi.org/) first.

## License

MIT — see [LICENSE](https://github.com/naginagiyev/dtr/blob/main/LICENSE).

## Links

- [GitHub](https://github.com/naginagiyev/dtr)
- [PyPI](https://pypi.org/project/dtrlib/)
