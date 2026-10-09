// SPDX-License-Identifier: Apache-2.0
//! Strict and completely pre-effect Phase 1 YAML-to-plan compiler.
//! No source, sink, runtime adapter, or output is opened here.

use serde_yaml_ng::{Mapping, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use tram_model::Datum;
use tram_registry::{p1_contracts, ConfigType, Registry, RegistryError, Role};

const YAML_CAP: usize = 256 * 1024;
const DEPTH_CAP: usize = 32;
pub const PLAN_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    MissingEnv(String),
    InvalidYaml,
    UnsupportedOption(String),
    UnsupportedPlugin(String),
    InvalidValue(String),
    UnsupportedExpression,
    ResourceLimit,
}
impl ConfigError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingEnv(_) => "CONFIG_MISSING_ENV",
            Self::InvalidYaml => "CONFIG_MALFORMED_YAML",
            Self::UnsupportedOption(_) => "CONFIG_UNSUPPORTED_OPTION",
            Self::UnsupportedPlugin(_) => "CONFIG_UNSUPPORTED_PLUGIN",
            Self::InvalidValue(_) => "CONFIG_INVALID_VALUE",
            Self::UnsupportedExpression => "CONFIG_UNSUPPORTED_EXPRESSION",
            Self::ResourceLimit => "CONFIG_RESOURCE_EXHAUSTED",
        }
    }
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}
impl Error for ConfigError {}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Datum),
    Field(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negative,
    Not,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    Rename(Vec<(String, String)>),
    AddField(Vec<(String, Expression)>),
    Filter(Expression),
    Drop(Vec<String>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub path: String,
    pub file_pattern: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Sink {
    pub path: String,
    pub filename_template: String,
    pub condition: Option<Expression>,
    pub transforms: Vec<Transform>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedPlan {
    pub contract_version: u32,
    pub ephemeral_only: bool,
    pub name: String,
    pub description: Option<String>,
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub sinks: Vec<Sink>,
}

/// Explicit environment, not ambient process environment. Substitution happens
/// before YAML parsing, identical to the Python loader ordering.
pub fn substitute_env(yaml: &str, env: &BTreeMap<String, String>) -> Result<String, ConfigError> {
    if yaml.len() > YAML_CAP {
        return Err(ConfigError::ResourceLimit);
    }
    let mut out = String::new();
    let mut rest = yaml;
    while let Some(start) = rest.find(concat!("$", "{")) {
        out.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        let close = rest.find('}').ok_or(ConfigError::InvalidYaml)?;
        let body = &rest[..close];
        let (key, default) = if let Some((key, default)) = body.split_once(":-") {
            (key, Some(default))
        } else {
            (body, None)
        };
        if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(ConfigError::InvalidYaml);
        }
        match env.get(key) {
            Some(value) => out.push_str(value),
            None => match default {
                Some(value) => out.push_str(value),
                None => return Err(ConfigError::MissingEnv(key.into())),
            },
        }
        if out.len() > YAML_CAP {
            return Err(ConfigError::ResourceLimit);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    if out.len() > YAML_CAP {
        return Err(ConfigError::ResourceLimit);
    }
    Ok(out)
}
fn check_depth(value: &Value, depth: usize) -> Result<(), ConfigError> {
    if depth > DEPTH_CAP {
        return Err(ConfigError::ResourceLimit);
    }
    match value {
        Value::Mapping(map) => {
            for (key, value) in map {
                if !key.is_string() {
                    return Err(ConfigError::InvalidYaml);
                }
                check_depth(value, depth + 1)?;
            }
        }
        Value::Sequence(items) => {
            for value in items {
                check_depth(value, depth + 1)?;
            }
        }
        Value::Tagged(_) => return Err(ConfigError::UnsupportedOption("yaml-tag".into())),
        _ => (),
    }
    Ok(())
}
fn key(name: &str) -> Value {
    Value::String(name.to_owned())
}
fn get<'a>(map: &'a Mapping, name: &str) -> Option<&'a Value> {
    map.get(key(name))
}
fn required<'a>(map: &'a Mapping, name: &str) -> Result<&'a Value, ConfigError> {
    get(map, name).ok_or_else(|| ConfigError::InvalidValue(name.into()))
}
fn map<'a>(value: &'a Value, where_: &str) -> Result<&'a Mapping, ConfigError> {
    value
        .as_mapping()
        .ok_or_else(|| ConfigError::InvalidValue(where_.into()))
}
fn string<'a>(value: &'a Value, where_: &str) -> Result<&'a str, ConfigError> {
    value
        .as_str()
        .ok_or_else(|| ConfigError::InvalidValue(where_.into()))
}
fn allowed(map: &Mapping, names: &[&str]) -> Result<(), ConfigError> {
    for key in map.keys() {
        let name = string(key, "key")?;
        if !names.contains(&name) {
            return Err(ConfigError::UnsupportedOption(name.into()));
        }
    }
    Ok(())
}
fn expected_string(map: &Mapping, name: &str, value: &str) -> Result<(), ConfigError> {
    if let Some(actual) = get(map, name) {
        if string(actual, name)? != value {
            return Err(ConfigError::UnsupportedOption(name.into()));
        }
    }
    Ok(())
}
fn expected_bool(map: &Mapping, name: &str, value: bool) -> Result<(), ConfigError> {
    if let Some(actual) = get(map, name) {
        if actual.as_bool() != Some(value) {
            return Err(ConfigError::UnsupportedOption(name.into()));
        }
    }
    Ok(())
}
fn expected_int(map: &Mapping, name: &str, value: i64) -> Result<(), ConfigError> {
    if let Some(actual) = get(map, name) {
        if actual.as_i64() != Some(value) {
            return Err(ConfigError::UnsupportedOption(name.into()));
        }
    }
    Ok(())
}
fn config_kind(value: &Value) -> Result<ConfigType, ConfigError> {
    match value {
        Value::String(_) => Ok(ConfigType::String),
        Value::Bool(_) => Ok(ConfigType::Boolean),
        Value::Number(n) if n.as_i64().is_some() || n.as_u64().is_some() => Ok(ConfigType::Integer),
        Value::Number(_) => Err(ConfigError::InvalidValue("float".into())),
        Value::Mapping(_) => Ok(ConfigType::Mapping),
        Value::Sequence(_) => Ok(ConfigType::Sequence),
        Value::Null => Ok(ConfigType::Null),
        Value::Tagged(_) => Err(ConfigError::InvalidYaml),
    }
}
fn validate_plugin(
    registry: &Registry,
    role: Role,
    name: &str,
    op: &str,
    value: &Mapping,
) -> Result<(), ConfigError> {
    let manifest = registry.select(role, name, op).map_err(|e| match e {
        RegistryError::UnknownPlugin { .. } => ConfigError::UnsupportedPlugin(name.into()),
        _ => ConfigError::UnsupportedOption(format!("{role:?}:{name}:{op}")),
    })?;
    let mut properties = BTreeMap::new();
    for (key, value) in value {
        properties.insert(string(key, "config key")?.into(), config_kind(value)?);
    }
    manifest.validate_config(&properties).map_err(|e| match e {
        RegistryError::UnknownConfiguration(name) => ConfigError::UnsupportedOption(name),
        RegistryError::WrongConfigurationType(name) | RegistryError::MissingConfiguration(name) => {
            ConfigError::InvalidValue(name)
        }
        _ => ConfigError::UnsupportedOption(name.into()),
    })
}
fn flat_name(name: &str) -> Result<String, ConfigError> {
    if name.is_empty() || name.len() > 512 || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(ConfigError::UnsupportedOption("field-path".into()));
    }
    Ok(name.into())
}
fn literal_path(v: &Value, name: &str) -> Result<String, ConfigError> {
    let path = string(v, name)?;
    if path.is_empty() || path.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(ConfigError::InvalidValue(name.into()));
    }
    Ok(path.into())
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Id(String),
    Int(i64),
    Str(String),
    Bool(bool),
    Add,
    Sub,
    Mul,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Open,
    Close,
}
fn lex(s: &str) -> Result<Vec<Token>, ConfigError> {
    if s.is_empty() || s.len() > 2048 {
        return Err(ConfigError::UnsupportedExpression);
    }
    let bytes = s.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let token = match bytes[i] {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let value = &s[start..i];
                match value {
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "True" | "true" => Token::Bool(true),
                    "False" | "false" => Token::Bool(false),
                    _ => Token::Id(value.into()),
                }
            }
            b'0'..=b'9' => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                Token::Int(
                    s[start..i]
                        .parse()
                        .map_err(|_| ConfigError::UnsupportedExpression)?,
                )
            }
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                let text_start = i;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        return Err(ConfigError::UnsupportedExpression);
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(ConfigError::UnsupportedExpression);
                }
                let value = s[text_start..i].to_owned();
                i += 1;
                Token::Str(value)
            }
            b'(' => {
                i += 1;
                Token::Open
            }
            b')' => {
                i += 1;
                Token::Close
            }
            b'+' => {
                i += 1;
                Token::Add
            }
            b'-' => {
                i += 1;
                Token::Sub
            }
            b'*' => {
                i += 1;
                Token::Mul
            }
            b'=' if bytes.get(i + 1) == Some(&b'=') => {
                i += 2;
                Token::Eq
            }
            b'!' if bytes.get(i + 1) == Some(&b'=') => {
                i += 2;
                Token::Ne
            }
            b'<' if bytes.get(i + 1) == Some(&b'=') => {
                i += 2;
                Token::Le
            }
            b'>' if bytes.get(i + 1) == Some(&b'=') => {
                i += 2;
                Token::Ge
            }
            b'<' => {
                i += 1;
                Token::Lt
            }
            b'>' => {
                i += 1;
                Token::Gt
            }
            _ => return Err(ConfigError::UnsupportedExpression),
        };
        tokens.push(token);
        if tokens.len() > 128 {
            return Err(ConfigError::UnsupportedExpression);
        }
    }
    Ok(tokens)
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}
impl Parser {
    fn take(&mut self, want: &Token) -> bool {
        if self.tokens.get(self.pos) == Some(want) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn parse(mut self) -> Result<Expression, ConfigError> {
        let ast = self.logical_or(0)?;
        if self.pos != self.tokens.len() {
            return Err(ConfigError::UnsupportedExpression);
        }
        Ok(ast)
    }
    fn logical_or(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        let mut a = self.logical_and(depth)?;
        while self.take(&Token::Or) {
            let b = self.logical_and(depth)?;
            a = Expression::Binary {
                op: BinaryOp::Or,
                left: Box::new(a),
                right: Box::new(b),
            };
        }
        Ok(a)
    }
    fn logical_and(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        let mut a = self.logical_not(depth)?;
        while self.take(&Token::And) {
            let b = self.logical_not(depth)?;
            a = Expression::Binary {
                op: BinaryOp::And,
                left: Box::new(a),
                right: Box::new(b),
            };
        }
        Ok(a)
    }
    // Python: arithmetic > comparison > not > and > or.
    // Unary 'not' must consume a comparison, not a single arithmetic atom.
    fn logical_not(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        if depth > 32 {
            return Err(ConfigError::UnsupportedExpression);
        }
        if self.take(&Token::Not) {
            let expr = self.logical_not(depth + 1)?;
            Ok(Expression::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            })
        } else {
            self.compare(depth)
        }
    }
    fn compare(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        let mut a = self.add(depth)?;
        let ops = [
            (Token::Eq, BinaryOp::Eq),
            (Token::Ne, BinaryOp::Ne),
            (Token::Lt, BinaryOp::Lt),
            (Token::Le, BinaryOp::Le),
            (Token::Gt, BinaryOp::Gt),
            (Token::Ge, BinaryOp::Ge),
        ];
        for (token, op) in ops {
            if self.take(&token) {
                let b = self.add(depth)?;
                a = Expression::Binary {
                    op,
                    left: Box::new(a),
                    right: Box::new(b),
                };
                break;
            }
        }
        Ok(a)
    }
    fn add(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        let mut a = self.mul(depth)?;
        loop {
            let op = if self.take(&Token::Add) {
                Some(BinaryOp::Add)
            } else if self.take(&Token::Sub) {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else { break };
            let b = self.mul(depth)?;
            a = Expression::Binary {
                op,
                left: Box::new(a),
                right: Box::new(b),
            };
        }
        Ok(a)
    }
    fn mul(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        let mut a = self.atom(depth)?;
        while self.take(&Token::Mul) {
            let b = self.atom(depth)?;
            a = Expression::Binary {
                op: BinaryOp::Mul,
                left: Box::new(a),
                right: Box::new(b),
            };
        }
        Ok(a)
    }
    fn atom(&mut self, depth: usize) -> Result<Expression, ConfigError> {
        if depth > 32 {
            return Err(ConfigError::UnsupportedExpression);
        }
        if self.take(&Token::Sub) {
            let expr = self.atom(depth + 1)?;
            return Ok(Expression::Unary {
                op: UnaryOp::Negative,
                expr: Box::new(expr),
            });
        }
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or(ConfigError::UnsupportedExpression)?;
        self.pos += 1;
        match token {
            Token::Id(value) => Ok(Expression::Field(flat_name(&value)?)),
            Token::Int(value) => Ok(Expression::Literal(Datum::Signed(value))),
            Token::Str(value) => Ok(Expression::Literal(Datum::String(value))),
            Token::Bool(value) => Ok(Expression::Literal(Datum::Boolean(value))),
            Token::Open => {
                let inside = self.logical_or(depth + 1)?;
                if !self.take(&Token::Close) {
                    return Err(ConfigError::UnsupportedExpression);
                }
                Ok(inside)
            }
            _ => Err(ConfigError::UnsupportedExpression),
        }
    }
}
fn expression(s: &str) -> Result<Expression, ConfigError> {
    Parser {
        tokens: lex(s)?,
        pos: 0,
    }
    .parse()
}
fn transforms(value: &Value, registry: &Registry) -> Result<Vec<Transform>, ConfigError> {
    let items = value
        .as_sequence()
        .ok_or_else(|| ConfigError::InvalidValue("transforms".into()))?;
    if items.len() > 64 {
        return Err(ConfigError::ResourceLimit);
    }
    let mut output = Vec::new();
    for item in items {
        let spec = map(item, "transform")?;
        let name = string(required(spec, "type")?, "type")?;
        validate_plugin(registry, Role::Transform, name, "stateless", spec)?;
        let transform = match name {
            "rename" => {
                let m = map(required(spec, "fields")?, "fields")?;
                let mut sources = BTreeSet::new();
                let mut targets = BTreeSet::new();
                let mut pairs = Vec::new();
                for (from, to) in m {
                    let from = flat_name(string(from, "rename source")?)?;
                    let to = flat_name(string(to, "rename target")?)?;
                    if from == to || !sources.insert(from.clone()) || !targets.insert(to.clone()) {
                        return Err(ConfigError::UnsupportedOption("rename conflict".into()));
                    }
                    pairs.push((from, to));
                }
                if !sources.is_disjoint(&targets) {
                    return Err(ConfigError::UnsupportedOption("overlapping rename".into()));
                }
                Transform::Rename(pairs)
            }
            "add_field" => {
                let m = map(required(spec, "fields")?, "fields")?;
                let mut pairs = Vec::new();
                for (field, expr) in m {
                    let name = flat_name(string(field, "add_field field")?)?;
                    pairs.push((name, expression(string(expr, "add_field expression")?)?));
                }
                Transform::AddField(pairs)
            }
            "filter" => Transform::Filter(expression(string(
                required(spec, "condition")?,
                "condition",
            )?)?),
            "drop" => {
                let items = required(spec, "fields")?
                    .as_sequence()
                    .ok_or_else(|| ConfigError::InvalidValue("drop fields".into()))?;
                let mut names = Vec::new();
                for f in items {
                    names.push(flat_name(string(f, "drop field")?)?);
                }
                Transform::Drop(names)
            }
            _ => return Err(ConfigError::UnsupportedPlugin(name.into())),
        };
        output.push(transform);
    }
    Ok(output)
}
fn serializer(value: &Value, registry: &Registry, op: &str) -> Result<(), ConfigError> {
    let m = map(value, "serializer")?;
    let name = string(required(m, "type")?, "type")?;
    validate_plugin(registry, Role::Serializer, name, op, m)
}
fn sinks(value: &Value, plural: bool, registry: &Registry) -> Result<Vec<Sink>, ConfigError> {
    // No single/list shape coercion: Python's 'sink' and 'sinks' are distinct.
    let items: Vec<&Value> = if plural {
        value
            .as_sequence()
            .ok_or_else(|| ConfigError::InvalidValue("sinks must be a sequence".into()))?
            .iter()
            .collect()
    } else {
        map(value, "sink must be one mapping")?;
        vec![value]
    };
    if items.is_empty() || items.len() > 2 {
        return Err(ConfigError::UnsupportedOption("sink count".into()));
    }
    let mut unique = BTreeSet::new();
    let mut output = Vec::new();
    for item in items {
        let m = map(item, "sink")?;
        let name = string(required(m, "type")?, "type")?;
        validate_plugin(registry, Role::Sink, name, "scratch-single", m)?;
        expected_string(m, "file_mode", "single")?;
        expected_bool(m, "overwrite", false)?;
        let path = literal_path(required(m, "path")?, "sink path")?;
        let filename = string(required(m, "filename_template")?, "filename_template")?;
        if filename.is_empty()
            || filename == "."
            || filename == ".."
            || filename.contains('/')
            || filename.contains('\\')
            || filename.contains('{')
            || filename.contains('}')
            || filename.contains("..")
            || filename.bytes().any(|b| b < 0x20 || b == 0x7f)
        {
            return Err(ConfigError::UnsupportedOption("unsafe filename".into()));
        }
        if !unique.insert((path.clone(), filename.to_owned())) {
            return Err(ConfigError::UnsupportedOption("duplicate sink".into()));
        }
        let condition = get(m, "condition")
            .map(|v| expression(string(v, "condition")?))
            .transpose()?;
        let branch = get(m, "transforms")
            .map(|v| transforms(v, registry))
            .transpose()?
            .unwrap_or_default();
        output.push(Sink {
            path,
            filename_template: filename.into(),
            condition,
            transforms: branch,
        });
    }
    Ok(output)
}
/// All validation occurs before returning a plan; no source/sink side effects.
pub fn compile_p1(
    yaml: &str,
    env: &BTreeMap<String, String>,
    registry: &Registry,
) -> Result<ValidatedPlan, ConfigError> {
    let resolved = substitute_env(yaml, env)?;
    let value: Value = serde_yaml_ng::from_str(&resolved).map_err(|_| ConfigError::InvalidYaml)?;
    check_depth(&value, 0)?;
    let root = map(&value, "root")?;
    let m = if let Some(v) = get(root, "pipeline") {
        allowed(root, &["pipeline", "version"])?;
        expected_string(root, "version", "1")?;
        map(v, "pipeline")?
    } else {
        root
    };
    allowed(
        m,
        &[
            "version",
            "name",
            "description",
            "enabled",
            "schedule",
            "source",
            "serializer_in",
            "serializer_out",
            "transforms",
            "sink",
            "sinks",
            "thread_workers",
            "parallel_sinks",
            "on_error",
            "retry_count",
            "retry_delay_seconds",
            "dlq",
            "delivery",
        ],
    )?;
    expected_string(m, "version", "1")?;
    let name = string(required(m, "name")?, "name")?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ConfigError::InvalidValue("name".into()));
    }
    let description = get(m, "description")
        .map(|v| string(v, "description").map(str::to_owned))
        .transpose()?;
    expected_bool(m, "enabled", true)?;
    let schedule = map(required(m, "schedule")?, "schedule")?;
    allowed(schedule, &["type"])?;
    if string(required(schedule, "type")?, "schedule.type")? != "manual" {
        return Err(ConfigError::UnsupportedOption("schedule.type".into()));
    }
    expected_int(m, "thread_workers", 1)?;
    expected_bool(m, "parallel_sinks", false)?;
    if string(required(m, "on_error")?, "on_error")? != "abort" {
        return Err(ConfigError::UnsupportedOption("on_error".into()));
    }
    for key in ["retry_count", "retry_delay_seconds"] {
        required(m, key)?;
        expected_int(m, key, 0)?;
    }
    if let Some(v) = get(m, "dlq") {
        if !v.is_null() {
            return Err(ConfigError::UnsupportedOption("dlq".into()));
        }
    }
    if let Some(v) = get(m, "delivery") {
        let d = map(v, "delivery")?;
        allowed(d, &["contract"])?;
        expected_string(d, "contract", "legacy")?;
    }
    let source = map(required(m, "source")?, "source")?;
    let source_type = string(required(source, "type")?, "source.type")?;
    validate_plugin(
        registry,
        Role::Source,
        source_type,
        "manual-readonly",
        source,
    )?;
    expected_bool(source, "recursive", false)?;
    let path = literal_path(required(source, "path")?, "source path")?;
    let pattern = get(source, "file_pattern")
        .map(|v| string(v, "file_pattern"))
        .transpose()?
        .unwrap_or("*");
    if pattern.is_empty()
        || pattern.contains('/')
        || pattern.contains('\\')
        || pattern.contains("..")
    {
        return Err(ConfigError::UnsupportedOption("file_pattern".into()));
    }
    serializer(required(m, "serializer_in")?, registry, "decode")?;
    if let Some(v) = get(m, "serializer_out") {
        serializer(v, registry, "encode")?;
    } else {
        registry
            .select(Role::Serializer, "json", "encode")
            .map_err(|_| ConfigError::UnsupportedOption("default json encode".into()))?;
    }
    let global = get(m, "transforms")
        .map(|v| transforms(v, registry))
        .transpose()?
        .unwrap_or_default();
    if get(m, "sink").is_some() && get(m, "sinks").is_some() {
        return Err(ConfigError::UnsupportedOption("sink and sinks".into()));
    }
    let destinations = if let Some(value) = get(m, "sink") {
        sinks(value, false, registry)?
    } else {
        sinks(required(m, "sinks")?, true, registry)?
    };
    Ok(ValidatedPlan {
        contract_version: PLAN_VERSION,
        ephemeral_only: true,
        name: name.into(),
        description,
        source: Source {
            path,
            file_pattern: pattern.into(),
        },
        transforms: global,
        sinks: destinations,
    })
}
pub fn compile_p1_builtin(
    yaml: &str,
    env: &BTreeMap<String, String>,
) -> Result<ValidatedPlan, ConfigError> {
    compile_p1(yaml, env, &p1_contracts())
}
