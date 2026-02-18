use rand::Rng;

/// Characters used for encoding the connection code.
/// Same as Terracotta: 0-9, A-Z excluding I and O (to avoid confusion).
static CHARS: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Room information derived from a connection code.
#[derive(Debug, Clone)]
pub struct Room {
    /// The human-readable connection code (e.g., "U/ABCD-1234-EFGH-5678")
    pub code: String,
    /// The EasyTier network name derived from the code
    pub network_name: String,
    /// The EasyTier network secret derived from the code
    pub network_secret: String,
}

/// Look up a character in the CHARS table, handling I->1 and O->0 substitutions.
fn lookup_char(ch: char) -> Option<u8> {
    let ch = match ch {
        'I' => '1',
        'O' => '0',
        _ => ch,
    };

    for (j, c) in CHARS.iter().enumerate() {
        if *c as char == ch {
            return Some(j as u8);
        }
    }

    None
}

/// Generate the code, network_name, and network_secret from a u128 value.
/// This is identical to Terracotta's `from_value` function.
fn from_value(value: u128) -> (String, String, String) {
    let mut code = String::with_capacity("U/XXXX-XXXX-XXXX-XXXX".len());
    code.push_str("U/");
    let mut network_name = String::with_capacity("scaffolding-mc-XXXX-XXXX".len());
    network_name.push_str("scaffolding-mc-");
    let mut network_secret = String::with_capacity("XXXX-XXXX".len());

    let mut value = value;
    for i in 0..16 {
        let v = CHARS[(value % 34) as usize] as char;
        value /= 34;

        if i == 4 || i == 8 || i == 12 {
            code.push('-');
        }
        code.push(v);

        if i < 8 {
            if i == 4 {
                network_name.push('-');
            }
            network_name.push(v);
        } else {
            if i == 12 {
                network_secret.push('-');
            }
            network_secret.push(v);
        }
    }

    assert_eq!(value, 0);

    (code, network_name, network_secret)
}

/// Create a new room with a randomly generated connection code.
/// The algorithm is identical to Terracotta's room creation.
pub fn create_room() -> Room {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.r#gen();
    let value = u128::from_be_bytes(bytes) % 34u128.pow(16);
    let value = value - value % 7;

    let (code, network_name, network_secret) = from_value(value);

    Room {
        code,
        network_name,
        network_secret,
    }
}

/// Parse a connection code string into a Room.
/// Supports the Terracotta format: U/XXXX-XXXX-XXXX-XXXX
pub fn parse_code(input: &str) -> Option<Room> {
    let code: Vec<char> = input.to_ascii_uppercase().chars().collect();
    if code.len() < "U/XXXX-XXXX-XXXX-XXXX".len() {
        return None;
    }

    let value: u128 = 'value: {
        'parse_segment: for code in code.windows("U/XXXX-XXXX-XXXX-XXXX".len()) {
            if code[0] != 'U' || code[1] != '/' {
                continue 'parse_segment;
            }

            let code = &code[2..];
            let mut value: u128 = 0;
            for i in (0.."XXXX-XXXX-XXXX-XXXX".len()).rev() {
                if i == 4 || i == 9 || i == 14 {
                    if code[i] != '-' {
                        continue 'parse_segment;
                    }
                } else {
                    match lookup_char(code[i]) {
                        Some(v) => value = value * 34 + v as u128,
                        None => continue 'parse_segment,
                    }
                }
            }
            if value % 7 == 0 {
                break 'value value;
            }
        }
        return None;
    };

    let (code, network_name, network_secret) = from_value(value);

    Some(Room {
        code,
        network_name,
        network_secret,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_parse() {
        for _ in 0..100 {
            let room = create_room();
            assert!(room.code.starts_with("U/"));
            assert_eq!(room.code.len(), "U/XXXX-XXXX-XXXX-XXXX".len());

            let parsed = parse_code(&room.code).expect("Failed to parse generated code");
            assert_eq!(room.code, parsed.code);
            assert_eq!(room.network_name, parsed.network_name);
            assert_eq!(room.network_secret, parsed.network_secret);
        }
    }

    #[test]
    fn test_i_o_substitution() {
        // I -> 1, O -> 0 should be handled
        if let Some(room) = parse_code("U/0000-0000-0000-0000") {
            let parsed = parse_code(&room.code.replace('0', "O"));
            assert!(parsed.is_some());
        }
    }
}
