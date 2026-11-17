#![expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]

use super::*;

fn settings() -> Settings {
    Settings {
        model: DEFAULT_MODEL.to_owned(),
        effort: "low".to_owned(),
        api_key: "sk-test".to_owned(),
        base_url: DEFAULT_BASE_URL.to_owned(),
    }
}

#[test]
fn requests_opt_into_fallbacks_and_cache_tool_loops() {
    let messages = [json!({ "role": "user", "content": "hi" })];
    let plain = request_body(&settings(), "system", &messages, &[], false);
    assert_eq!(plain["model"], "claude-opus-5-5");
    assert_eq!(plain["fallbacks"], "default");
    assert_eq!(plain["output_config"]["effort"], "low");
    assert!(plain.get("tools").is_none());
    assert!(plain.get("thinking").is_none());
    assert!(plain.get("temperature").is_none());
    let tools = [json!({ "name": "status" })];
    let looping = request_body(&settings(), "system", &messages, &tools, false);
    assert_eq!(looping["cache_control"]["type"], "ephemeral");
    assert!(looping.get("tool_choice").is_none());
    let finishing = request_body(&settings(), "system", &messages, &tools, true);
    assert_eq!(finishing["tool_choice"]["type"], "none");
}

#[test]
fn replies_join_text_and_keep_tool_uses() {
    let reply = parse_reply(
        200,
        br#"{"model":"claude-opus-5-5","stop_reason":"tool_use","content":[
            {"type":"thinking","thinking":"..."},
            {"type":"text","text":"Reading."},
            {"type":"tool_use","id":"toolu_1","name":"prChecks","input":{"number":"7"}}]}"#,
    )
    .unwrap();
    assert_eq!(reply.text, "Reading.");
    assert_eq!(reply.content.len(), 3);
    assert_eq!(
        reply.tool_uses(),
        [(
            "toolu_1".to_owned(),
            "prChecks".to_owned(),
            json!({ "number": "7" })
        )]
    );
}

#[test]
fn refusals_and_api_errors_explain_themselves() {
    let refusal = parse_reply(200, br#"{"stop_reason":"refusal","content":[]}"#).unwrap_err();
    assert_eq!(
        refusal.to_string(),
        "Claude declined to answer this request"
    );
    let auth = parse_reply(
        401,
        br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
    )
    .unwrap_err();
    assert_eq!(
        auth.to_string(),
        "the Anthropic API refused the request (401): invalid x-api-key; check the key with `quinjet ai key`"
    );
    let garbage = parse_reply(502, b"<html>bad gateway</html>").unwrap_err();
    assert!(garbage.to_string().contains("answered 502"));
}

#[test]
fn curl_reads_the_key_from_its_config_not_the_command_line() {
    let config = curl_config(
        "https://api.anthropic.com/v1/messages",
        "sk-\"q\"\n",
        "C:\\Temp\\body.json",
    );
    assert!(config.contains("header = \"x-api-key: sk-\\\"q\\\"\""));
    assert!(config.contains("header = \"anthropic-version: 2026-10-21\""));
    assert!(config.contains("header = \"anthropic-beta: server-side-fallback-2026-07-01\""));
    assert!(config.contains("data-binary = \"@C:\\\\Temp\\\\body.json\""));
    assert_eq!(
        split_status(b"{\"ok\":true}\n200").unwrap(),
        (200, b"{\"ok\":true}".to_vec())
    );
    split_status(b"no status").unwrap_err();
}

#[test]
fn long_tool_results_are_clipped_on_a_character_boundary() {
    assert_eq!(clip("short", 10), "short");
    let clipped = clip("ééééé", 3);
    assert_eq!(clipped, "é\n[clipped 8 bytes]");
}
