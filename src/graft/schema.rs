use serde_json::{Map, Value, json};

use super::receiver::{Kind, Method};

pub(crate) const REPOSITORY_PROPERTY: &str = "repository";

pub(crate) fn input_schema(method: &Method, repository: bool) -> Value {
    let mut properties = Map::new();
    for param in &method.params {
        drop(properties.insert(
            param.name.clone(),
            json!({
                "type": if param.kind == Kind::Switch { "boolean" } else { "string" },
                "description": param.description(),
            }),
        ));
    }
    if repository {
        drop(properties.insert(
            REPOSITORY_PROPERTY.to_owned(),
            json!({
                "type": "string",
                "description": "Path of the Git repository to use. Optional; defaults to the folder the agent started in",
            }),
        ));
    }
    let required: Vec<&str> = method
        .params
        .iter()
        .filter(|param| param.required)
        .map(|param| param.name.as_str())
        .collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

pub(crate) fn mcp_tool(method: &Method) -> Value {
    json!({
        "name": method.name,
        "description": describe(method),
        "inputSchema": input_schema(method, true),
        "annotations": {
            "readOnlyHint": method.access == super::receiver::Access::Read && !method.is_ai(),
            "openWorldHint": method.is_ai() || method.path.first().is_some_and(|group| group == "pr" || group == "stack"),
        },
    })
}

pub(crate) fn describe(method: &Method) -> String {
    let mut text = method.about.clone();
    if text.is_empty() {
        text = method.path.join(" ");
    }
    text.push_str(". Runs `quinjet ");
    text.push_str(&method.path.join(" "));
    text.push_str(" --json`");
    if method.is_ai() {
        text.push_str(" and asks Quinjet's configured Claude model");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_type_switches_and_list_required_parameters() {
        let receiver = crate::cli::graft_receiver(false);
        let checks = receiver.find("prChecks").unwrap();
        let schema = input_schema(checks, true);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["number"]));
        assert_eq!(schema["properties"]["refresh"]["type"], "boolean");
        assert_eq!(schema["properties"]["number"]["type"], "string");
        assert!(schema["properties"][REPOSITORY_PROPERTY].is_object());
        assert!(
            input_schema(checks, false)["properties"]
                .get(REPOSITORY_PROPERTY)
                .is_none()
        );
        let tool = mcp_tool(receiver.find("status").unwrap());
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert!(
            tool["description"]
                .as_str()
                .unwrap()
                .ends_with("Runs `quinjet status --json`")
        );
    }
}
