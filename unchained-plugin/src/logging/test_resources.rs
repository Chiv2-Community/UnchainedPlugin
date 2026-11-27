use serde_yaml::Value;

fn load(source: &str, name: &str) -> Value {
    let cases: Value = serde_yaml::from_str(source).unwrap();
    cases.get(name).unwrap_or_else(|| panic!("Missing logging fixture: {name}")).clone()
}

pub fn encoder(name: &str) -> Value {
    load(include_str!("../../test-resources/logging/encoders.yaml"), name)
}

pub fn config(name: &str) -> Value {
    load(include_str!("../../test-resources/logging/configs.yaml"), name)
}

pub fn appender(name: &str) -> Value {
    load(include_str!("../../test-resources/logging/appenders.yaml"), name)
}
