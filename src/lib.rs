use pyo3::prelude::*;
use pyo3::types::PyList;
use std::sync::Arc;

pub mod syntax;
pub mod textcompiler;

use syntax::CompilerSyntax;
use textcompiler::{DynamicValue, TextCompiler};

#[pymodule]
fn _dtc(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyTextCompiler>()?;
    m.add_class::<PyCompilerSyntax>()?;
    Ok(())
}

#[pyclass(name = "CompilerSyntax")]
#[derive(Clone)]
struct PyCompilerSyntax {
    inner: CompilerSyntax,
}

#[pymethods]
impl PyCompilerSyntax {
    #[new]
    #[pyo3(signature = (
        function_prefix=None,
        include_prefix=None,
        variable_prefix=None,
        function_name_pattern=None,
        variable_name_pattern=None,
    ))]
    fn new(
        function_prefix: Option<String>,
        include_prefix: Option<String>,
        variable_prefix: Option<String>,
        function_name_pattern: Option<String>,
        variable_name_pattern: Option<String>,
    ) -> Self {
        let mut inner = CompilerSyntax::default();
        if let Some(value) = function_prefix {
            inner.function_prefix = value;
        }
        if let Some(value) = include_prefix {
            inner.include_prefix = value;
        }
        if let Some(value) = variable_prefix {
            inner.variable_prefix = value;
        }
        if let Some(value) = function_name_pattern {
            inner.function_name_pattern = value;
        }
        if let Some(value) = variable_name_pattern {
            inner.variable_name_pattern = value;
        }
        Self { inner }
    }
}

#[pyclass(name = "TextCompiler")]
struct PyTextCompiler {
    inner: TextCompiler,
}

#[pymethods]
impl PyTextCompiler {
    #[new]
    #[pyo3(signature = (syntax=None))]
    fn new(syntax: Option<PyCompilerSyntax>) -> Self {
        let syntax = syntax.map(|value| value.inner);
        Self {
            inner: TextCompiler::new(syntax),
        }
    }

    fn compile(&self, text: &str) -> String {
        self.inner.compile(text)
    }

    #[pyo3(signature = (text, file_name))]
    fn compile_with_file(&self, text: &str, file_name: &str) -> String {
        self.inner.compile_with_file(text, file_name)
    }

    fn set_debug_mode(&mut self, debug_mode: bool) {
        self.inner.set_debug_mode(debug_mode);
    }

    fn set_arg(&mut self, name: &str, value: DynamicValue) {
        self.inner.args.insert(name.to_string(), value);
    }

    fn clear_args(&mut self) {
        self.inner.args.clear();
    }

    fn add_function(&mut self, full_function_name: &str, callable: PyObject) -> PyResult<()> {
        let callable = Arc::new(callable);
        self.inner.add_function(full_function_name, move |args| {
            Python::with_gil(|py| -> DynamicValue {
                let py_callable = (&*callable).as_ref(py);
                let py_args = PyList::empty(py);
                for arg in args {
                    if py_args.append(arg.clone().into_py(py)).is_err() {
                        return DynamicValue::Null;
                    }
                }
                match py_callable.call1((py_args,)) {
                    Ok(result) => result.extract().unwrap_or(DynamicValue::Null),
                    Err(_) => DynamicValue::Null,
                }
            })
        });
        Ok(())
    }
}

impl IntoPy<PyObject> for DynamicValue {
    fn into_py(self, py: Python) -> PyObject {
        match self {
            DynamicValue::Null => py.None(),
            DynamicValue::Bool(b) => b.into_py(py),
            DynamicValue::Number(n) => n.into_py(py),
            DynamicValue::String(s) => s.into_py(py),
            DynamicValue::List(l) => {
                let list = PyList::empty(py);
                for item in l {
                    list.append(item.into_py(py)).expect("append");
                }
                list.into()
            }
            DynamicValue::Dict(d) => {
                let dict = pyo3::types::PyDict::new(py);
                for (k, v) in d {
                    dict.set_item(k, v.into_py(py)).expect("set_item");
                }
                dict.into()
            }
        }
    }
}

impl<'source> FromPyObject<'source> for DynamicValue {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        if ob.is_none() {
            return Ok(DynamicValue::Null);
        }
        if let Ok(s) = ob.extract::<String>() {
            return Ok(DynamicValue::String(s));
        }
        if let Ok(b) = ob.extract::<bool>() {
            return Ok(DynamicValue::Bool(b));
        }
        if let Ok(n) = ob.extract::<f64>() {
            return Ok(DynamicValue::Number(n));
        }
        if let Ok(n) = ob.extract::<i64>() {
            return Ok(DynamicValue::Number(n as f64));
        }
        if let Ok(list) = ob.extract::<Vec<DynamicValue>>() {
            return Ok(DynamicValue::List(list));
        }
        if let Ok(dict) = ob.extract::<std::collections::HashMap<String, DynamicValue>>() {
            return Ok(DynamicValue::Dict(dict));
        }
        Ok(DynamicValue::Null)
    }
}
