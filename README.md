# dtc (Dynamic Text Compiler)

Python bindings for the Dynamic Text Compiler, built with Rust and PyO3.

## Development

```bash
pip install maturin
maturin develop
```

```python
from dtc import TextCompiler

compiler = TextCompiler()
print(compiler.compile("~fn:my.fn()"))
```

## Publish to PyPI

```bash
maturin build --release
maturin publish
```

Create accounts on [PyPI](https://pypi.org) and [TestPyPI](https://test.pypi.org) first. Use `maturin publish --repository testpypi` for a trial upload.
"# dtc" 
