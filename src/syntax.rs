use regex::escape;

#[derive(Clone, Debug)]
pub struct CompilerSyntax {
    pub function_prefix: String,
    pub include_prefix: String,
    pub variable_prefix: String,
    pub function_name_pattern: String,
    pub variable_name_pattern: String,
}

impl Default for CompilerSyntax {
    fn default() -> Self {
        Self {
            function_prefix: "~fn:".to_string(),
            include_prefix: "~include:".to_string(),
            variable_prefix: "$".to_string(),
            function_name_pattern: r"[\w.]+".to_string(),
            variable_name_pattern: r"[A-Za-z_]\w*".to_string(),
        }
    }
}

impl CompilerSyntax {
    pub fn get_function_pattern(&self) -> String {
        let escaped_prefix = escape(&self.function_prefix);
        format!(r#"{}({})\(([^)]*)\)"#, escaped_prefix, self.function_name_pattern)
    }

    pub fn get_variable_pattern(&self) -> String {
        let escaped_prefix = escape(&self.variable_prefix);
        format!(r#"{}({})"#, escaped_prefix, self.variable_name_pattern)
    }

    pub fn get_include_pattern(&self) -> String {
        let escaped_prefix = escape(&self.include_prefix);
        format!(
            r#"{}\s*("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s]+)"#,
            escaped_prefix
        )
    }
}
