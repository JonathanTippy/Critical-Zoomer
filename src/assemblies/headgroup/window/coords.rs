use rug::Integer;
use crate::assemblies::headgroup::window::sampling::ZoomerCommand;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::*;
use crate::utils::ObjectivePosAndZoom;

pub fn f64_to_intexp(v: f64) -> IntExp {
    if v == 0.0 {
        return IntExp::ZERO;
    }
    let sign = if v < 0.0 { -1 } else { 1 };
    let mut av = v.abs();
    let mut exp = 0i32;
    while av < 1.0 {
        av *= 2.0;
        exp -= 1;
    }
    while av >= 2.0 {
        av /= 2.0;
        exp += 1;
    }
    let mantissa = (av * (1u64 << 52) as f64).round() as i64;
    IntExp {
        val: Integer::from(sign) * Integer::from(mantissa)
        , exp: exp - 52
    }
}

pub fn decimal_str_to_intexp(s: &str) -> Option<IntExp> {
    let v: f64 = s.trim().parse().ok()?;
    Some(f64_to_intexp(v))
}

/// UL location so viewport center is at (center_re, center_im) in math coords.
/// Stored imag is negated (SetPos convention).
pub fn ul_for_center(
    center_re: IntExp
    , center_im: IntExp
    , zoom_pot: i32
    , screen: (u32, u32)
) -> ObjectivePosAndZoom {
    let half_w = IntExp {
        val: Integer::from(screen.0 / 2)
        , exp: -zoom_pot
    }.shift(-PIXELS_PER_UNIT_POT);
    let half_h = IntExp {
        val: Integer::from(screen.1 / 2)
        , exp: -zoom_pot
    }.shift(-PIXELS_PER_UNIT_POT);
    ObjectivePosAndZoom {
        pos: (
            center_re - half_w
            // stored pos.1 is negated imag of UL; center_im is mathematical imag
            , IntExp::ZERO - center_im - half_h
        )
        , zoom_pot
    }
}

/// Viewport center in mathematical (re, im) from UL location.
pub fn viewport_center(loc: &ObjectivePosAndZoom, screen: (u32, u32)) -> (IntExp, IntExp) {
    let half_w = IntExp {
        val: Integer::from(screen.0 / 2)
        , exp: -loc.zoom_pot
    }.shift(-PIXELS_PER_UNIT_POT);
    let half_h = IntExp {
        val: Integer::from(screen.1 / 2)
        , exp: -loc.zoom_pot
    }.shift(-PIXELS_PER_UNIT_POT);
    let re = loc.pos.0.clone() + half_w;
    // loc.pos.1 is negated UL imag → math imag of UL is -pos.1; center imag = UL_im - half_h
    let im = IntExp::ZERO - loc.pos.1.clone() - half_h;
    (re, im)
}

pub fn parse_complex(input: &str) -> Option<(IntExp, IntExp)> {
    let mut s = input.trim().to_string();
    for ch in ['(', ')', '[', ']', '{', '}'] {
        s = s.replace(ch, "");
    }
    s = s.replace(' ', "");
    if s.is_empty() {
        return None;
    }
    if let Some(idx) = s.find(',') {
        let re = decimal_str_to_intexp(&s[..idx])?;
        let im = decimal_str_to_intexp(&s[idx + 1..])?;
        return Some((re, im));
    }
    let lower = s.to_lowercase();
    if !lower.contains('i') {
        return None;
    }

    // Forms: a+bi, a-bi, bi+a, bi-a, bi, a+i, i+a, etc.
    let normalized = normalize_complex_string(&lower)?;
    parse_normalized_complex(&normalized)
}

fn normalize_complex_string(s: &str) -> Option<String> {
    if s.ends_with('i') {
        return Some(s.to_string());
    }
    // Imag-leading: Ni±M or i±M → M±Ni
    if let Some(i_pos) = s.find('i') {
        let imag_part = &s[..i_pos];
        let rest = &s[i_pos + 1..];
        if rest.is_empty() {
            return Some(format!("{}i", if imag_part.is_empty() { "1" } else { imag_part }));
        }
        let imag_coeff = if imag_part.is_empty() {
            "1".to_string()
        } else if imag_part == "+" || imag_part == "-" {
            format!("{}1", imag_part)
        } else {
            imag_part.to_string()
        };
        if rest.starts_with('+') || rest.starts_with('-') {
            let re = &rest[1..];
            let re_sign = if rest.starts_with('-') { "-" } else { "" };
            let imag_sign = if imag_coeff.starts_with('-') { "-" } else { "+" };
            let imag_abs = imag_coeff.trim_start_matches(['+', '-']);
            return Some(format!("{}{}{}{}i", re_sign, re, imag_sign, imag_abs));
        }
    }
    None
}

fn parse_normalized_complex(s: &str) -> Option<(IntExp, IntExp)> {
    let without_i = s.trim_end_matches('i');
    if without_i.is_empty() || without_i == "+" || without_i == "-" {
        let im = if without_i == "-" {
            IntExp::from(-1)
        } else {
            IntExp::from(1)
        };
        return Some((IntExp::ZERO, im));
    }
    if let Some(plus) = without_i.rfind('+') {
        let (re_s, im_s) = without_i.split_at(plus);
        let im_raw = &im_s[1..];
        let im = if im_raw.is_empty() {
            IntExp::from(1)
        } else {
            decimal_str_to_intexp(im_raw)?
        };
        let re = if re_s.is_empty() {
            IntExp::ZERO
        } else {
            decimal_str_to_intexp(re_s)?
        };
        return Some((re, im));
    }
    if without_i.len() > 1 {
        if let Some(minus) = without_i[1..].rfind('-') {
            let idx = minus + 1;
            let (re_s, im_s) = without_i.split_at(idx);
            let im = if im_s == "-" {
                IntExp::from(-1)
            } else {
                decimal_str_to_intexp(im_s)?
            };
            let re = if re_s.is_empty() {
                IntExp::ZERO
            } else {
                decimal_str_to_intexp(re_s)?
            };
            return Some((re, im));
        }
    }
    let im = decimal_str_to_intexp(without_i)?;
    Some((IntExp::ZERO, im))
}

pub fn commands_from_navigate_line(line: &str) -> Option<Vec<ZoomerCommand>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // requirements: space or comma separators both valid ("0, 0" / "0 0")
    let normalized = line.replace(',', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.len() < 2 {
        return parse_complex(line).map(|(re, im)| {
            vec![ZoomerCommand::NavigateTo { real: re, imag: im, pot: 0 }]
        });
    }
    let re = decimal_str_to_intexp(parts[0])?;
    let im = decimal_str_to_intexp(parts[1])?;
    let pot = if parts.len() >= 3 {
        parts[2].parse().ok()?
    } else {
        0
    };
    Some(vec![ZoomerCommand::NavigateTo { real: re, imag: im, pot }])
}

pub fn commands_from_goto_line(line: &str) -> Option<Vec<ZoomerCommand>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // requirements: space or comma separators both valid ("0, 0" / "0 0")
    let normalized = line.replace(',', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.len() < 2 {
        return parse_complex(line).map(|(re, im)| {
            vec![ZoomerCommand::SetPos { real: re, imag: im }]
        });
    }
    let re = decimal_str_to_intexp(parts[0])?;
    let im = decimal_str_to_intexp(parts[1])?;
    let mut cmds = vec![ZoomerCommand::SetPos { real: re, imag: im }];
    if parts.len() >= 3 {
        let pot: i32 = parts[2].parse().ok()?;
        cmds.push(ZoomerCommand::SetZoom { pot });
    }
    Some(cmds)
}

pub fn goto_line_is_valid(line: &str) -> bool {
    commands_from_goto_line(line).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_comma_pair() {
        let (re, im) = parse_complex("1.5, -2").unwrap();
        assert!((re.to_f64() - 1.5).abs() < 1e-9);
        assert!((im.to_f64() + 2.0).abs() < 1e-9);
    }

    #[test]
    fn parse_plus_i_form() {
        let (re, im) = parse_complex("3+4i").unwrap();
        assert!((re.to_f64() - 3.0).abs() < 1e-9);
        assert!((im.to_f64() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn parse_imag_leading_parens() {
        // requirements: (5i + 6) = (6 + 5i)
        let (re, im) = parse_complex("(5i + 6)").unwrap();
        assert!((re.to_f64() - 6.0).abs() < 1e-9);
        assert!((im.to_f64() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn ul_for_center_zero_centers_viewport() {
        let loc = ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, (800, 480));
        let (re, im) = viewport_center(&loc, (800, 480));
        let re_f = re.to_f64();
        let im_f = im.to_f64();
        assert!(re_f.abs() < 1e-6, "re={re_f}");
        assert!(im_f.abs() < 1e-6, "im={im_f}");
    }

    #[test]
    fn empty_goto_invalid() {
        assert!(!goto_line_is_valid(""));
        assert!(!goto_line_is_valid("   "));
        assert!(goto_line_is_valid("0, 0"));
    }
}
