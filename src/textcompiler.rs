use crate::syntax::CompilerSyntax;
use regex::{Captures, Regex};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum DynamicValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<DynamicValue>),
    Dict(HashMap<String, DynamicValue>),
}

impl DynamicValue {
    fn to_output_string(&self) -> String {
        match self {
            DynamicValue::Null => String::new(),
            DynamicValue::Bool(value) => value.to_string(),
            DynamicValue::Number(value) => {
                if value.fract() == 0.0 {
                    (*value as i64).to_string()
                } else {
                    value.to_string()
                }
            }
            DynamicValue::String(value) => value.clone(),
            DynamicValue::List(values) => {
                let items: Vec<String> = values.iter().map(|value| value.to_output_string()).collect();
                format!("[{}]", items.join(", "))
            }
            DynamicValue::Dict(values) => {
                let items: Vec<String> = values
                    .iter()
                    .map(|(key, value)| format!("{}: {}", key, value.to_output_string()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }
}

type CompilerFunction = Arc<dyn Fn(Vec<DynamicValue>) -> DynamicValue + Send + Sync>;

#[derive(Clone, Debug)]
pub struct CompilerError {
    pub file_name: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for CompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}: {}",
            self.file_name, self.line, self.column, self.message
        )
    }
}

pub struct TextCompiler {
    pub syntax: CompilerSyntax,
    pub args: HashMap<String, DynamicValue>,
    pub debug_mode: bool,
    functions: HashMap<String, CompilerFunction>,
    callable_cache: RefCell<HashMap<String, Option<CompilerFunction>>>,
    args_parse_cache: RefCell<HashMap<String, Vec<DynamicValue>>>,
}

impl Default for TextCompiler {
    fn default() -> Self {
        Self::new(None)
    }
}

impl TextCompiler {
    fn split_top_level(&self, text: &str, separator: char) -> Result<Vec<String>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut parts: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut bracket_depth = 0i32;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut escape_next = false;

        for ch in text.chars() {
            if escape_next {
                current.push(ch);
                escape_next = false;
                continue;
            }

            if ch == '\\' {
                current.push(ch);
                escape_next = true;
                continue;
            }

            if ch == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                current.push(ch);
                continue;
            }

            if ch == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                current.push(ch);
                continue;
            }

            if !in_single_quote && !in_double_quote {
                if "([{".contains(ch) {
                    bracket_depth += 1;
                } else if ")]}".contains(ch) {
                    bracket_depth -= 1;
                    if bracket_depth < 0 {
                        return Err("Unexpected closing bracket".to_string());
                    }
                } else if ch == separator && bracket_depth == 0 {
                    parts.push(current.trim().to_string());
                    current.clear();
                    continue;
                }
            }

            current.push(ch);
        }

        if bracket_depth != 0 {
            return Err("Unclosed bracket".to_string());
        }
        if in_single_quote || in_double_quote {
            return Err("Unclosed string literal".to_string());
        }
        if escape_next {
            return Err("Trailing escape character".to_string());
        }

        parts.push(current.trim().to_string());
        Ok(parts)
    }

    fn strip_quotes(&self, value: &str) -> String {
        if value.len() >= 2 {
            let first = value.chars().next().unwrap_or_default();
            let last = value.chars().last().unwrap_or_default();
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                return value[1..value.len() - 1].to_string();
            }
        }
        value.to_string()
    }

    fn parse_list(&self, argument_expression: &str) -> Option<DynamicValue> {
        let trimmed = argument_expression.trim();
        if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
            return None;
        }

        let inner = &trimmed[1..trimmed.len() - 1];
        let parts = self.split_top_level(inner, ',').ok()?;
        let mut values: Vec<DynamicValue> = Vec::new();

        for part in parts {
            if part.is_empty() {
                values.push(DynamicValue::Null);
            } else {
                values.push(self.parse_argument(&part));
            }
        }

        Some(DynamicValue::List(values))
    }

    fn parse_dict(&self, argument_expression: &str) -> Option<DynamicValue> {
        let trimmed = argument_expression.trim();
        if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
            return None;
        }

        let inner = &trimmed[1..trimmed.len() - 1];
        let entries = self.split_top_level(inner, ',').ok()?;
        let mut map: HashMap<String, DynamicValue> = HashMap::new();

        for entry in entries {
            if entry.is_empty() {
                continue;
            }
            let pair = self.split_top_level(&entry, ':').ok()?;
            if pair.len() != 2 {
                return None;
            }
            let key = self.strip_quotes(pair[0].trim());
            if key.is_empty() {
                return None;
            }
            let value_expression = pair[1].trim();
            let value = if value_expression.is_empty() {
                DynamicValue::Null
            } else {
                self.parse_argument(value_expression)
            };
            map.insert(key, value);
        }

        Some(DynamicValue::Dict(map))
    }

    pub fn new(syntax: Option<CompilerSyntax>) -> Self {
        Self {
            syntax: syntax.unwrap_or_default(),
            args: HashMap::new(),
            debug_mode: false,
            functions: HashMap::new(),
            callable_cache: RefCell::new(HashMap::new()),
            args_parse_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn set_debug_mode(&mut self, debug_mode: bool) {
        self.debug_mode = debug_mode;
    }

    pub fn add_function<F>(&mut self, full_function_name: &str, function: F)
    where
        F: Fn(Vec<DynamicValue>) -> DynamicValue + Send + Sync + 'static,
    {
        self.functions
            .insert(full_function_name.to_string(), Arc::new(function));
        self.callable_cache.borrow_mut().remove(full_function_name);
    }

    fn get_callable(&self, full_function_name: &str) -> Option<CompilerFunction> {
        if let Some(cached) = self.callable_cache.borrow().get(full_function_name) {
            return cached.clone();
        }

        let resolved = self.functions.get(full_function_name).cloned();
        self.callable_cache
            .borrow_mut()
            .insert(full_function_name.to_string(), resolved.clone());
        resolved
    }

    fn parse_argument(&self, argument_expression: &str) -> DynamicValue {
        let trimmed = argument_expression.trim();
        let variable_regex = Regex::new(&format!(
            "^{}$",
            self.syntax.get_variable_pattern()
        ))
        .expect("invalid variable regex");

        if let Some(captures) = variable_regex.captures(trimmed) {
            if let Some(variable_name) = captures.get(1) {
                if let Some(value) = self.args.get(variable_name.as_str()) {
                    return value.clone();
                }
            }
        }

        if let Ok(parsed_bool) = trimmed.parse::<bool>() {
            return DynamicValue::Bool(parsed_bool);
        }

        if let Ok(parsed_number) = trimmed.parse::<f64>() {
            return DynamicValue::Number(parsed_number);
        }

        if let Some(list_value) = self.parse_list(trimmed) {
            return list_value;
        }

        if let Some(dict_value) = self.parse_dict(trimmed) {
            return dict_value;
        }

        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            return DynamicValue::String(trimmed[1..trimmed.len() - 1].to_string());
        }

        DynamicValue::String(trimmed.to_string())
    }

    fn parse_args(&self, args_string: &str) -> Result<Vec<DynamicValue>, (String, usize)> {
        if let Some(cached) = self.args_parse_cache.borrow().get(args_string) {
            return Ok(cached.clone());
        }

        if args_string.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut parsed_args = Vec::new();
        let mut current_token = String::new();
        let mut bracket_depth = 0i32;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut escape_next = false;

        for (index, character) in args_string.char_indices() {
            if escape_next {
                current_token.push(character);
                escape_next = false;
                continue;
            }

            if character == '\\' {
                current_token.push(character);
                escape_next = true;
                continue;
            }

            if character == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                current_token.push(character);
                continue;
            }

            if character == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                current_token.push(character);
                continue;
            }

            let inside_string = in_single_quote || in_double_quote;
            if !inside_string {
                if "([{".contains(character) {
                    bracket_depth += 1;
                } else if ")]}".contains(character) {
                    bracket_depth -= 1;
                    if bracket_depth < 0 {
                        return Err((
                            "Unexpected closing bracket in function arguments".to_string(),
                            index,
                        ));
                    }
                } else if character == ',' && bracket_depth == 0 {
                    let token_expression = current_token.trim();
                    if token_expression.is_empty() {
                        parsed_args.push(DynamicValue::Null);
                    } else {
                        parsed_args.push(self.parse_argument(token_expression));
                    }
                    current_token.clear();
                    continue;
                }
            }

            current_token.push(character);
        }

        let token_expression = current_token.trim();
        if !token_expression.is_empty() {
            parsed_args.push(self.parse_argument(token_expression));
        }

        if bracket_depth != 0 {
            return Err((
                "Unclosed bracket in function arguments".to_string(),
                args_string.len(),
            ));
        }
        if in_single_quote || in_double_quote {
            return Err((
                "Unclosed string literal in function arguments".to_string(),
                args_string.len(),
            ));
        }
        if escape_next {
            return Err((
                "Trailing escape character in function arguments".to_string(),
                args_string.len(),
            ));
        }

        self.args_parse_cache
            .borrow_mut()
            .insert(args_string.to_string(), parsed_args.clone());
        Ok(parsed_args)
    }

    fn get_line_and_column(&self, text: &str, byte_index: usize) -> (usize, usize) {
        let safe_index = byte_index.min(text.len());
        let mut line = 1usize;
        let mut line_start = 0usize;

        for (index, ch) in text.char_indices() {
            if index >= safe_index {
                break;
            }
            if ch == '\n' {
                line += 1;
                line_start = index + ch.len_utf8();
            }
        }

        let column = text[line_start..safe_index].chars().count() + 1;
        (line, column)
    }

    fn build_compiler_error(
        &self,
        text: &str,
        file_name: &str,
        byte_index: usize,
        message: String,
    ) -> CompilerError {
        let (line, column) = self.get_line_and_column(text, byte_index);
        CompilerError {
            file_name: file_name.to_string(),
            line,
            column,
            message,
        }
    }

    fn replace_function_call(
        &self,
        text: &str,
        file_name: &str,
        captures: &Captures,
    ) -> Result<String, CompilerError> {
        let whole_match = captures
            .get(0)
            .expect("regex capture should include full match");
        let full_function_name = captures.get(1).map_or("", |value| value.as_str());
        let args_string = captures.get(2).map_or("", |value| value.as_str());

        let Some(function) = self.get_callable(full_function_name) else {
            return Ok(whole_match.as_str().to_string());
        };

        let parsed_args = match self.parse_args(args_string) {
            Ok(value) => value,
            Err((message, relative_index)) => {
                let args_start = whole_match.start()
                    + self.syntax.function_prefix.len()
                    + full_function_name.len()
                    + 1;
                return Err(self.build_compiler_error(
                    text,
                    file_name,
                    args_start + relative_index,
                    message,
                ));
            }
        };
        Ok(function(parsed_args).to_output_string())
    }

    fn resolve_include_path(&self, include_target: &str, file_name: &str) -> PathBuf {
        let mut include_name = include_target.trim().to_string();
        if include_name.len() >= 2 {
            let first = include_name.chars().next().unwrap_or_default();
            let last = include_name.chars().last().unwrap_or_default();
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                include_name = include_name[1..include_name.len() - 1].to_string();
            }
        }

        let include_path = PathBuf::from(include_name);
        if include_path.is_absolute() {
            return include_path;
        }

        if file_name != "<input>" {
            let parent = Path::new(file_name).parent().unwrap_or_else(|| Path::new("."));
            return parent.join(include_path);
        }

        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(include_path)
    }

    fn normalize_path(path: &Path) -> String {
        match fs::canonicalize(path) {
            Ok(value) => value.to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        }
    }

    fn process_includes(
        &self,
        text: &str,
        file_name: &str,
        include_stack: &[String],
    ) -> Result<String, CompilerError> {
        let include_pattern = self.syntax.get_include_pattern();
        let include_regex = Regex::new(&include_pattern).expect("invalid include regex");
        let mut result = String::new();
        let mut last_end = 0usize;

        for captures in include_regex.captures_iter(text) {
            let whole_match = captures
                .get(0)
                .expect("regex capture should include full match");
            result.push_str(&text[last_end..whole_match.start()]);

            let include_target = captures.get(1).map_or("", |value| value.as_str());
            let include_path = self.resolve_include_path(include_target, file_name);
            let include_key = Self::normalize_path(&include_path);

            if include_stack.contains(&include_key) {
                let error = self.build_compiler_error(
                    text,
                    file_name,
                    whole_match.start(),
                    format!("Circular include detected for '{}'", include_path.to_string_lossy()),
                );
                if self.debug_mode {
                    return Err(error);
                }
                result.push_str(whole_match.as_str());
                last_end = whole_match.end();
                continue;
            }

            let included_text = match fs::read_to_string(&include_path) {
                Ok(value) => value,
                Err(error) => {
                    let include_error = self.build_compiler_error(
                        text,
                        file_name,
                        whole_match.start(),
                        format!(
                            "Include failed for '{}': {}",
                            include_path.to_string_lossy(),
                            error
                        ),
                    );
                    if self.debug_mode {
                        return Err(include_error);
                    }
                    result.push_str(whole_match.as_str());
                    last_end = whole_match.end();
                    continue;
                }
            };

            let mut nested_stack = include_stack.to_vec();
            nested_stack.push(include_key);
            let included_compiled = self.compile_with_stack(
                &included_text,
                &include_path.to_string_lossy(),
                &nested_stack,
            );

            match included_compiled {
                Ok(value) => result.push_str(&value),
                Err(error) => {
                    if self.debug_mode {
                        return Err(error);
                    }
                    result.push_str(whole_match.as_str());
                }
            }

            last_end = whole_match.end();
        }

        result.push_str(&text[last_end..]);
        Ok(result)
    }

    fn compile_with_stack(
        &self,
        text: &str,
        file_name: &str,
        include_stack: &[String],
    ) -> Result<String, CompilerError> {
        let processed_text = self.process_includes(text, file_name, include_stack)?;
        let function_pattern = self.syntax.get_function_pattern();
        let function_regex = Regex::new(&function_pattern).expect("invalid function regex");
        let mut result = String::new();
        let mut last_end = 0usize;

        for captures in function_regex.captures_iter(&processed_text) {
            let whole_match = captures
                .get(0)
                .expect("regex capture should include full match");
            result.push_str(&processed_text[last_end..whole_match.start()]);

            match self.replace_function_call(&processed_text, file_name, &captures) {
                Ok(replacement) => result.push_str(&replacement),
                Err(error) => {
                    if self.debug_mode {
                        return Err(error);
                    }
                    result.push_str(whole_match.as_str());
                }
            }

            last_end = whole_match.end();
        }

        result.push_str(&processed_text[last_end..]);
        Ok(result)
    }

    pub fn compile(&self, text: &str) -> String {
        self.compile_with_file(text, "<input>")
    }

    pub fn compile_with_file(&self, text: &str, file_name: &str) -> String {
        let mut include_stack = Vec::new();
        if file_name != "<input>" {
            include_stack.push(Self::normalize_path(Path::new(file_name)));
        }
        self.args_parse_cache.borrow_mut().clear();

        match self.compile_with_stack(text, file_name, &include_stack) {
            Ok(value) => value,
            Err(error) => {
                if self.debug_mode {
                    panic!("{}", error);
                }
                text.to_string()
            }
        }
    }
}
