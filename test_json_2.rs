fn main() {
    let raw = r#"{
        "1": {"content": "napıyon"},
        "2": {"content": "What's up"}
    }"#;
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    let mut text = String::new();
    fn gather(v: &serde_json::Value, t: &mut String) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(serde_json::Value::String(c)) = m.get("content") {
                    t.push_str(c);
                    t.push_str(" ");
                }
                for (_, val) in m {
                    gather(val, t);
                }
            }
            serde_json::Value::Array(a) => {
                for val in a { gather(val, t); }
            }
            _ => ()
        }
    }
    gather(&v, &mut text);
    println!("Text: {:?}", text);
    let lower_q = "napıyon".to_lowercase();
    let lower_full = text.to_lowercase();
    println!("Found idx: {:?}", lower_full.find(&lower_q));
}
