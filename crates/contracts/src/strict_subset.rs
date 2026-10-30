//! Minimal candidate structured-output profiles for the future P5 provider
//! capability test. Offline validation does NOT certify an OpenRouter route.
use serde_json::{Map, Value, json};

fn primitive(kind:&str)->Value {json!({"type":kind})}
fn enum_value(values:Value)->Value {json!({"type":"string","enum":values})}
fn object(properties:Map<String,Value>)->Value{
    let required:Vec<String>=properties.keys().cloned().collect();
    json!({"type":"object","additionalProperties":false,
        "properties":properties,"required":required})
}
fn fields(items:Vec<(&str,Value)>)->Map<String,Value>{
    items.into_iter().map(|(k,v)|(k.to_owned(),v)).collect()
}
fn allocation()->Value{
    object(fields(vec![
        ("staff",primitive("integer")),("repairs",primitive("integer")),
        ("access",primitive("integer"))]))
}
fn conditions()->Value{
    json!({"type":"array","items":object(fields(vec![
        ("kind_id",primitive("string")),
        ("family",enum_value(json!(["staffing","repairs","access"]))),
        ("units",primitive("integer")),
        ("due_quarter",primitive("integer")),
    ]))})
}
fn shared(stage:&str,ministry:&str)->Vec<(&'static str,Value)>{
    vec![
        ("schema_version",json!({"type":"integer","enum":[1]})),
        ("stage",enum_value(json!([stage]))),
        ("meeting_id",primitive("string")),
        ("proposal_version",primitive("string")),
        ("ministry",enum_value(json!([ministry]))),
        ("position",enum_value(json!(["support","conditional","oppose"]))),
        ("preferred_allocation",allocation()),
        ("conditions",conditions()),
        ("evidence_ids",json!({"type":"array","items":primitive("string")})),
        ("rationale_summary",primitive("string")),
    ]
}
pub fn first(ministry:&str,menu:bool)->Value{
    let mut a=shared("first",ministry);
    let origin=if menu{
        object(fields(vec![
            ("kind",enum_value(json!(["menu"]))),
            ("menu_id",primitive("string")),
        ]))
    }else{
        object(fields(vec![("kind",enum_value(json!(["new_typed"])))]))
    };
    a.push(("counterproposal_source",origin));
    object(fields(a))
}
pub fn final_action(ministry:&str)->Value{
    let mut a=shared("final",ministry);
    if ministry!="finance"{
        a.push(("implementation_effort_permille",primitive("integer")));
    }
    object(fields(a))
}
/// Artifact contains nine stage/role profiles. One schema is selected per
/// request. All limits omitted here MUST be enforced by checked Rust parsing.
pub fn profiles()->Value{
    json!({
        "format_version":1,
        "status":"candidate_not_provider_certified",
        "required_provider_verification":"P5-01",
        "profiles":{
            "first_new_typed_finance":first("finance",false),
            "first_menu_finance":first("finance",true),
            "first_new_typed_education":first("education",false),
            "first_menu_education":first("education",true),
            "first_new_typed_infrastructure":first("infrastructure",false),
            "first_menu_infrastructure":first("infrastructure",true),
            "final_finance":final_action("finance"),
            "final_education":final_action("education"),
            "final_infrastructure":final_action("infrastructure"),
        }
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    fn inspect(value:&Value){
        if let Some(obj)=value.as_object(){
            let permitted=["type","properties","required","additionalProperties",
                "enum","items"];
            for (key,v) in obj{
                assert!(permitted.contains(&key.as_str()),"unsupported keyword {key}");
                if key=="properties" {
                    for nested in v.as_object().unwrap().values(){inspect(nested);}
                }else if key=="items"{inspect(v);}
            }
        }
    }
    #[test]
    fn every_candidate_is_in_small_strict_schema_subset(){
        let content=profiles();
        let p=content["profiles"].as_object().unwrap();
        assert_eq!(p.len(),9);
        for schema in p.values(){inspect(schema);}
        assert!(p["final_education"]["properties"].get("implementation_effort_permille").is_some());
        assert!(p["final_finance"]["properties"].get("implementation_effort_permille").is_none());
        assert!(p["first_menu_education"]["properties"].get("counterproposal_source").is_some());
        assert!(p["final_education"]["properties"].get("counterproposal_source").is_none());
        assert_eq!(p["first_new_typed_education"]["additionalProperties"],false);
    }
}
