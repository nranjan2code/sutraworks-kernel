use std::string::{String, ToString};
use std::vec::Vec;

#[derive(Debug, Clone)]
pub struct AppManifest {
    pub app_name: String,
    pub description: String,
    pub triggers: Vec<Trigger>,
    pub flow: Vec<FlowStep>,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub input_pattern: String,
    // intent_override removed for host test simplicity or mocked if needed
}

#[derive(Debug, Clone)]
pub struct FlowStep {
    pub id: String,
    pub goal: String,
    pub inputs: Vec<String>,
    pub condition: Option<String>,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidFormat,
    UnexpectedToken,
    MissingField(&'static str),
}

impl AppManifest {
    pub fn parse(source: &str) -> Result<AppManifest, ParseError> {
        let mut app_name = None;
        let mut description = None;
        let mut triggers = Vec::new();
        let mut flow = Vec::new();
        
        let mut current_section = "";
        let mut in_list_item = false;
        
        for line in source.lines() {
            let trim = line.trim();
            if trim.is_empty() || trim.starts_with('#') { continue; }
            
            let indent = line.len() - line.trim_start().len();
            
            if indent == 0 {
                if let Some((key, val)) = split_key_val(trim) {
                    match key {
                        "app_name" => app_name = Some(strip_quotes(val)),
                        "description" => description = Some(strip_quotes(val)),
                        "triggers" => current_section = "triggers",
                        "flow" => current_section = "flow",
                        _ => {}
                    }
                    in_list_item = false;
                } else if trim.ends_with(':') {
                     match trim.trim_end_matches(':') {
                        "triggers" => current_section = "triggers",
                        "flow" => current_section = "flow",
                        _ => {}
                     }
                     in_list_item = false;
                }
            } else if indent >= 2 {
                if trim.starts_with("- ") {
                    in_list_item = true;
                    match current_section {
                        "triggers" => triggers.push(Trigger { input_pattern: String::new() }),
                        "flow" => flow.push(FlowStep { id: String::new(), goal: String::new(), inputs: Vec::new(), condition: None }),
                        _ => {}
                    }
                    
                    let content = &trim[2..];
                    if !content.is_empty() {
                         parse_field(content, current_section, &mut triggers, &mut flow)?;
                    }
                } else {
                    if in_list_item {
                        parse_field(trim, current_section, &mut triggers, &mut flow)?;
                    }
                }
            }
        }
        
        Ok(AppManifest {
            app_name: app_name.ok_or(ParseError::MissingField("app_name"))?,
            description: description.unwrap_or_default(),
            triggers,
            flow,
        })
    }
}

fn split_key_val(s: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() == 2 {
        Some((parts[0].trim(), parts[1].trim()))
    } else {
        None
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') {
        s[1..s.len()-1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_field(line: &str, section: &str, triggers: &mut Vec<Trigger>, flow: &mut Vec<FlowStep>) -> Result<(), ParseError> {
    if let Some((key, val)) = split_key_val(line) {
        let val_str = strip_quotes(val);
        match section {
            "triggers" => {
                if let Some(trigger) = triggers.last_mut() {
                    match key {
                        "input" => trigger.input_pattern = val_str,
                        _ => {}
                    }
                }
            },
            "flow" => {
                if let Some(step) = flow.last_mut() {
                    match key {
                        "id" => step.id = val_str,
                        "goal" => step.goal = val_str,
                        "inputs" => {
                            let content = val.trim_start_matches('[').trim_end_matches(']');
                            step.inputs = content.split(',').map(|s| strip_quotes(s.trim())).filter(|s| !s.is_empty()).collect();
                        },
                        "condition" => step.condition = Some(val_str),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_manifest() {
        let source = r#"
app_name: "Smart Doorknob"
description: "A demo app"
triggers:
  - input: "Face detected"
flow:
  - id: "step1"
    goal: "Unlock"
    inputs: ["trigger.image"]
"#;
        let manifest = AppManifest::parse(source).expect("Failed to parse");
        assert_eq!(manifest.app_name, "Smart Doorknob");
        assert_eq!(manifest.triggers.len(), 1);
        assert_eq!(manifest.triggers[0].input_pattern, "Face detected");
        assert_eq!(manifest.flow.len(), 1);
        assert_eq!(manifest.flow[0].id, "step1");
        assert_eq!(manifest.flow[0].goal, "Unlock");
        assert_eq!(manifest.flow[0].inputs, vec!["trigger.image"]);
    }
}
