use pgl_domain::MarkedText;

fn main() {
    let html = "Ele disse <b>não</b> e saiu.";
    let mt = pgl_domain::MarkedText::from_html(html).unwrap();
    
    println!("Raw: {}", mt.raw);
    println!("Placeholders: {:?}", mt.placeholders);
    
    // Debug the restore method
    let mut chars = mt.raw.chars().peekable();
    let mut result = String::new();
    
    while let Some(c) = chars.next() {
        if c == '⟦' {
            let mut ph_num = String::new();
            chars.next(); // consume '⟦'
            
            while let Some(&next_c) = chars.peek() {
                chars.next();
                if next_c == '⟧' {
                    break;
                }
                ph_num.push(next_c);
            }
            
            println!("Found placeholder: '{}'", ph_num);
            
            if let Some(replacement) = mt.placeholders.get(&ph_num) {
                println!("Found replacement: '{}'", replacement);
            } else {
                println!("NOT FOUND: '{}'", ph_num);
            }
        } else {
            // just skip
        }
    }
}
