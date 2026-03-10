fn main() {
    let td = r#"{"node_1773098474367_3ihptd8uk":{"parentid":null,"role":"user","content":"napıyon","extranodeshtml":"","images":[],"children":["node_1773098"]}}"#;
    let val = serde_json::from_str::<serde_json::Value>(td);
    println!("parse ok? {}", val.is_ok());
    if let Ok(value) = val {
        let mut full_text = String::new();
        fn gather_text(v: &serde_json::Value, t: &mut String) {
            match v {
                serde_json::Value::Object(map) => {
                    if let Some(serde_json::Value::String(c)) = map.get("content") {
                        if !c.trim().is_empty() {
                            t.push_str(c.trim());
                            t.push_str(" ");
                        }
                    }
                    for (_, val) in map {
                        gather_text(val, t);
                    }
                }
                serde_json::Value::Array(arr) => {
                    for val in arr {
                        gather_text(val, t);
                    }
                }
                _ => {}
            }
        }
        gather_text(&value, &mut full_text);
        println!("Full text: '{}'", full_text);
        let q = "napıyon";
        let lower_full = full_text.to_lowercase();
        let lower_q = q.to_lowercase();
        println!("Found: {:?}", lower_full.find(&lower_q));
    }
}
