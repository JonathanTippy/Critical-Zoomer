use rug::Integer;
use crate::assemblies::headgroup::window::sampling::ZoomerCommand;
use crate::intexp::*;

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

pub fn parse_complex(input: &str) -> Option<(IntExp, IntExp)> {
    let mut s = input.trim().to_string();
    for ch in ['(', ')', '[', ']', '{', '}'] {
        s = s.replace(ch, "");
    }
    s = s.replace(' ', "");
    if let Some(idx) = s.find(',') {
        let re = decimal_str_to_intexp(&s[..idx])?;
        let im = decimal_str_to_intexp(&s[idx + 1..])?;
        return Some((re, im));
    }
    let lower = s.to_lowercase();
    if !lower.contains('i') {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() >= 2 {
            return Some((decimal_str_to_intexp(parts[0])?, decimal_str_to_intexp(parts[1])?));
        }
        return None;
    }
    let without_i = s.trim_end_matches(['i', 'I']);
    if let Some(plus) = without_i.rfind('+') {
        let (re_s, im_s) = without_i.split_at(plus);
        let im = decimal_str_to_intexp(&im_s[1..])?;
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
            let im = decimal_str_to_intexp(im_s)?;
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
    let parts: Vec<&str> = line.split_whitespace().collect();
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
    let parts: Vec<&str> = line.split_whitespace().collect();
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
