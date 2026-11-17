use serde_json::{Map, Value};

use super::receiver::{Kind, Method};

impl Method {
    pub(crate) fn argv(&self, values: &Map<String, Value>) -> Vec<String> {
        let mut options = Vec::new();
        let mut positionals = Vec::new();
        for param in &self.params {
            let Some(value) = values.get(&param.name).filter(|value| present(value)) else {
                continue;
            };
            if param.kind == Kind::Switch {
                if enabled(value) {
                    options.push(param.flag.clone());
                }
                continue;
            }
            for item in items(value, param.list) {
                if param.kind == Kind::Option {
                    options.push(param.flag.clone());
                    options.push(item);
                } else {
                    positionals.push(item);
                }
            }
        }
        let mut argv = self.path.clone();
        argv.extend(options);
        argv.push("--json".to_owned());
        if !positionals.is_empty() {
            argv.push("--".to_owned());
            argv.extend(positionals);
        }
        argv
    }
}

const fn present(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::String(text) => !text.is_empty(),
        _ => true,
    }
}

fn enabled(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_u64() == Some(1),
        Value::String(text) => matches!(text.as_str(), "1" | "true" | "yes"),
        _ => false,
    }
}

fn items(value: &Value, list: bool) -> Vec<String> {
    let text = |value: &Value| match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    if !list {
        return vec![text(value)];
    }
    match value {
        Value::Array(values) => values.iter().map(text).collect::<Vec<_>>(),
        other => text(other).split_whitespace().map(str::to_owned).collect(),
    }
    .into_iter()
    .filter(|item| !item.is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::super::receiver::{Access, Param};
    use super::*;

    fn words(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn arguments_become_the_same_command_line_the_module_builds() {
        let method = Method {
            name: "prChecks".to_owned(),
            path: words(&["pr", "checks"]),
            about: String::new(),
            access: Access::Read,
            params: vec![
                Param {
                    name: "number".to_owned(),
                    help: String::new(),
                    kind: Kind::Positional,
                    flag: String::new(),
                    list: false,
                    required: true,
                    choices: Vec::new(),
                    default: None,
                },
                Param {
                    name: "refresh".to_owned(),
                    help: String::new(),
                    kind: Kind::Switch,
                    flag: "--refresh".to_owned(),
                    list: false,
                    required: false,
                    choices: Vec::new(),
                    default: None,
                },
                Param {
                    name: "paths".to_owned(),
                    help: String::new(),
                    kind: Kind::Option,
                    flag: "--path".to_owned(),
                    list: true,
                    required: false,
                    choices: Vec::new(),
                    default: None,
                },
            ],
        };
        let values = serde_json::json!({"number": 42, "refresh": "true", "paths": "a b"});
        let Value::Object(values) = values else {
            panic!("not an object");
        };
        assert_eq!(
            method.argv(&values),
            words(&[
                "pr",
                "checks",
                "--refresh",
                "--path",
                "a",
                "--path",
                "b",
                "--json",
                "--",
                "42"
            ])
        );
        let empty = serde_json::json!({"number": "", "refresh": false});
        let Value::Object(empty) = empty else {
            panic!("not an object");
        };
        assert_eq!(method.argv(&empty), words(&["pr", "checks", "--json"]));
        assert!(!method.is_ai());
    }
}
