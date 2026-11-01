use pgl_domain::MarkedText;

fn main() {
    let html = "Ele disse <b>não</b> e saiu.";
    let mt = pgl_domain::MarkedText::from_html(html).unwrap();
    
    println!("Raw: {}", mt.raw);
    println!("Placeholders: {:?}", mt.placeholders);
    
    let restored = mt.restore().unwrap();
    println!("Restored: {}", restored);
}
