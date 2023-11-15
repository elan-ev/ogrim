// This is ugly: the following code needs to be used by the macro at compile
// time, but also by the library at run time. The "proper" way would be to add
// yet another crate that both crates can depend on. But that's super annoying,
// it's already bad enough that two crates are required for all of this. So
// screw it, I just `include!` this code in both code bases.

fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_name_start_char(first) && chars.all(is_name_char)
}

fn is_name_start_char(c: char) -> bool {
    matches!(c,
        ':'
        | 'A'..='Z'
        | '_'
        | 'a'..='z'
        | '\u{C0}'..='\u{D6}'
        | '\u{D8}'..='\u{F6}'
        | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}'
        | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}'
    )
}

fn is_name_char(c: char) -> bool {
    is_name_start_char(c) || matches!(c,
        '-'
        | '.'
        | '0'..='9'
        | '\u{B7}'
        | '\u{0300}'..='\u{036F}'
        | '\u{203F}'..='\u{2040}'
    )
}
