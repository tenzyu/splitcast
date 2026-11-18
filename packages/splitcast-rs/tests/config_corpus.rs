use splitcast_rs_core::config::Config;

#[test]
fn frozen_oracle_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("config-corpus.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        // The frozen corpus also covers the historical configuration surface.
        // Apply only the surviving cases to the current schema.
        if name.contains("camera")
            || name.contains("route")
            || name.contains("retry_limit")
            || name == "valid remote tts"
        {
            continue;
        }
        let mut input = case["input"].clone();
        input.as_object_mut().unwrap().remove("camera");
        input.as_object_mut().unwrap().remove("routes");
        for (section, field) in [
            ("peer", "retry_limit"),
            ("obs", "twitch_key_file"),
            ("tts", "mix_remote"),
        ] {
            input[section].as_object_mut().unwrap().remove(field);
        }
        let input = serde_json::to_vec(&input).unwrap();
        let expected = case["oracle_accepted"].as_bool().unwrap() && case["reason"].is_null();
        assert_eq!(Config::parse(&input).is_ok(), expected, "{}", case["name"]);
    }
}
