// read delivery.md for project context
use rug::Integer;
use crate::assemblies::headgroup::window::sampling::ZoomerCommand;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::{IntExp, ObjectivePosAndZoom};

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
// r[impl cz.ui.location-readout+2]
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
// r[impl cz.ui.location-readout+2]
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

/// Compact decimal for the location HUD (avoid IntExp Display's "n...." truncation).
// r[impl cz.ui.location-readout+2]
pub fn format_intexp_readout(v: &IntExp) -> String {
    let f = f64::from(v.clone());
    if !f.is_finite() {
        return "nan".to_string();
    }
    if f == 0.0 {
        return "0".to_string();
    }
    let abs = f.abs();
    if abs >= 1e6 || abs < 1e-4 {
        format!("{f:.6e}")
    } else {
        let s = format!("{f:.12}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Read-only location field: center re/im + magnification pot (requirements).
// r[impl cz.ui.location-readout+2]
pub fn format_location_readout(re: &IntExp, im: &IntExp, zoom_pot: i32) -> String {
    let im_s = format_intexp_readout(im);
    let im_part = if im_s.starts_with('-') {
        format!("{im_s}i")
    } else {
        format!("+ {im_s}i")
    };
    format!(
        "{} {}  mag 2^{}"
        , format_intexp_readout(re)
        , im_part
        , zoom_pot
    )
}

/// Write location text to the OS clipboard (survives app restart).
pub fn write_location_clipboard(text: &str) {
    if let Ok(mut clip) = arboard::Clipboard::new() {
        let _ = clip.set_text(text.to_owned());
    }
}

/// Read plain text from the OS clipboard for goto paste.
pub fn read_location_clipboard() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

// r[impl cz.ui.coords-parse+2]
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
        // Skip the first Unicode scalar (not byte 1) so non-ASCII input cannot panic.
        let first_end = without_i.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
        if first_end < without_i.len() {
            if let Some(minus) = without_i[first_end..].rfind('-') {
                let idx = first_end + minus;
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
    }
    let im = decimal_str_to_intexp(without_i)?;
    Some((IntExp::ZERO, im))
}

pub fn commands_from_goto_line(line: &str) -> Option<Vec<ZoomerCommand>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // Harness / ctl "home": restore startup HOME_POSITION framing (UL + zoom).
    if line.eq_ignore_ascii_case("home") {
        return Some(vec![
            ZoomerCommand::MoveTo {
                x: IntExp::from(crate::constants::HOME_POSITION.0),
                y: IntExp::from(crate::constants::HOME_POSITION.1),
            },
            ZoomerCommand::SetZoom {
                pot: crate::constants::HOME_POSITION.2,
            },
        ]);
    }
    let (re, im, pot) = parse_location_or_pair(line)?;
    let pot = pot?;
    // SetZoom must precede SetPos: ul_for_center uses the active zoom pot.
    // Emitting SetPos first made the same pasted location land differently
    // depending on the caller's current magnification (B-GOTO-2).
    let mut cmds = Vec::new();
    cmds.push(ZoomerCommand::SetZoom { pot });
    cmds.push(ZoomerCommand::SetPos { real: re, imag: im });
    Some(cmds)
}

/// Parse HUD readout (`a ± bi mag 2^N`) or legacy `re im [pot]` / `re, im`.
/// Returns `(re, im, zoom_pot)` — pot is `Some` when the line named a magnification.
fn parse_location_or_pair(line: &str) -> Option<(IntExp, IntExp, Option<i32>)> {
    let (body, mag_pot) = split_mag_suffix(line);
    if let Some((re, im)) = parse_complex(body) {
        return Some((re, im, mag_pot));
    }
    // Legacy: space/comma separated re im [pot]
    let normalized = body.replace(',', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let re = decimal_str_to_intexp(parts[0])?;
    let im = decimal_str_to_intexp(parts[1].trim_end_matches('i'))?;
    let pot = if let Some(p) = mag_pot {
        Some(p)
    } else if parts.len() >= 3 {
        Some(parts[2].parse().ok()?)
    } else {
        None
    };
    Some((re, im, pot))
}

/// Split trailing `mag 2^N` (location HUD / Copy format) from the complex body.
fn split_mag_suffix(line: &str) -> (&str, Option<i32>) {
    let lower = line.to_lowercase();
    let Some(idx) = lower.rfind("mag") else {
        return (line, None);
    };
    // Require a word boundary-ish break so "imaginary" does not match.
    if idx > 0 {
        let prev = line.as_bytes()[idx - 1];
        if prev.is_ascii_alphanumeric() {
            return (line, None);
        }
    }
    let after = line[idx + 3..].trim();
    let Some(pot) = parse_mag_token(after) else {
        return (line, None);
    };
    (line[..idx].trim(), Some(pot))
}

fn parse_mag_token(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("2^") {
        return rest.parse().ok();
    }
    if let Some(rest) = s.strip_prefix("2**") {
        return rest.parse().ok();
    }
    None
}

pub fn goto_line_is_valid(line: &str) -> bool {
    commands_from_goto_line(line).is_some()
}

/// D-UI-1 / REQ-CTRL-APPLY: Apply stays enabled whenever the field is valid,
/// including when it already equals the current viewport location.
// r[impl cz.ui.coords-apply+1]
pub fn apply_button_enabled(line_valid: bool, _already_at_location: bool) -> bool {
    line_valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use crate::assemblies::headgroup::window::sampling::SamplingContext;
    use crate::assemblies::headgroup::window::transforms::transform;
    use crate::constants::TEST_SCREEN_RES;

    // r[verify cz.ui.coords-parse+2]
    #[test]
    fn parse_comma_pair() {
        let (re, im) = parse_complex("1.5, -2").unwrap();
        assert!((f64::from(re) - 1.5).abs() < 1e-9);
        assert!((f64::from(im) + 2.0).abs() < 1e-9);
    }

    // r[verify cz.ui.coords-parse+2]
    #[test]
    fn parse_plus_i_form() {
        let (re, im) = parse_complex("3+4i").unwrap();
        assert!((f64::from(re) - 3.0).abs() < 1e-9);
        assert!((f64::from(im) - 4.0).abs() < 1e-9);
    }

    // r[verify cz.ui.coords-parse+2]
    #[test]
    fn parse_imag_leading_parens() {
        // requirements: (5i + 6) = (6 + 5i)
        let (re, im) = parse_complex("(5i + 6)").unwrap();
        assert!((f64::from(re) - 6.0).abs() < 1e-9);
        assert!((f64::from(im) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn move_up_increases_math_imag() {
        // Arrow/W up uses negative stored pixels_y; stored pos.1 is negated imag.
        let mut loc = ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, TEST_SCREEN_RES);
        let (_, im0) = viewport_center(&loc, TEST_SCREEN_RES);
        let im0 = f64::from(im0);
        let delta = IntExp { val: Integer::from(64), exp: 0 };
        loc.pos.1 = loc.pos.1.clone() + (IntExp::from(0) - delta)
            .shift(-loc.zoom_pot)
            .shift(-PIXELS_PER_UNIT_POT);
        let (_, im1) = viewport_center(&loc, TEST_SCREEN_RES);
        let im1 = f64::from(im1);
        assert!(im1 > im0, "Up must look toward +imag; im0={im0} im1={im1}");
    }

    #[test]
    fn grab_drag_down_increases_math_imag() {
        let start = ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, TEST_SCREEN_RES);
        let (_, im0) = viewport_center(&start, TEST_SCREEN_RES);
        let im0 = f64::from(im0);
        let objective_drag_y = IntExp { val: Integer::from(32), exp: 0 }
            .shift(-start.zoom_pot)
            .shift(-PIXELS_PER_UNIT_POT);
        let mut loc = start;
        loc.pos.1 = loc.pos.1 - objective_drag_y;
        let (_, im1) = viewport_center(&loc, TEST_SCREEN_RES);
        let im1 = f64::from(im1);
        assert!(im1 > im0, "mouse-down grab must raise math imag; im0={im0} im1={im1}");
    }

    // r[verify cz.ui.location-readout+2]
    #[test]
    fn ul_for_center_zero_centers_viewport() {
        let loc = ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, TEST_SCREEN_RES);
        let (re, im) = viewport_center(&loc, TEST_SCREEN_RES);
        let re_f = f64::from(re);
        let im_f = f64::from(im);
        assert!(re_f.abs() < 1e-6, "re={re_f}");
        assert!(im_f.abs() < 1e-6, "im={im_f}");
    }

    // r[verify cz.ui.location-readout+2]
    #[test]
    fn location_readout_includes_magnification_pot() {
        let loc = ul_for_center(IntExp::from(1), IntExp::from(-1), 5, TEST_SCREEN_RES);
        assert_eq!(loc.zoom_pot, 5);
        let (re, im) = viewport_center(&loc, TEST_SCREEN_RES);
        assert!((f64::from(re.clone()) - 1.0).abs() < 1e-6);
        assert!((f64::from(im.clone()) + 1.0).abs() < 1e-6);
        let text = format_location_readout(&re, &im, loc.zoom_pot);
        assert!(
            text.contains("mag 2^5"),
            "HUD string must include magnification; got {text}"
        );
    }

    // r[verify cz.ui.location-readout+2]
    #[test]
    fn location_readout_tracks_center_not_ul() {
        let loc = ul_for_center(IntExp::from(2), IntExp::ZERO, 0, TEST_SCREEN_RES);
        // UL real is left of center; readout must report center=2, not UL.
        assert!(f64::from(loc.pos.0.clone()) < 2.0);
        let (re, _) = viewport_center(&loc, TEST_SCREEN_RES);
        assert!((f64::from(re) - 2.0).abs() < 1e-6);
    }

    // r[verify cz.ui.location-readout+2]
    #[test]
    fn location_readout_string_has_center_and_mag() {
        let loc = ul_for_center(IntExp::from(0), IntExp::from(0), 3, TEST_SCREEN_RES);
        let (re, im) = viewport_center(&loc, TEST_SCREEN_RES);
        let text = format_location_readout(&re, &im, loc.zoom_pot);
        assert!(text.contains('0'), "got {text}");
        assert!(text.contains("mag 2^3"), "got {text}");
        assert!(!text.contains("..."), "must not use truncated IntExp Display; got {text}");
    }

    // r[verify cz.ui.location-readout+2]
    #[test]
    fn format_intexp_readout_avoids_ellipsis_truncation() {
        let v = f64_to_intexp(0.5);
        let s = format_intexp_readout(&v);
        assert!(!s.contains("..."), "got {s}");
        assert!(s.contains('5') || s.contains("0.5") || s.starts_with('5'), "got {s}");
    }

    #[test]
    fn empty_goto_invalid() {
        assert!(!goto_line_is_valid(""));
        assert!(!goto_line_is_valid("   "));
        assert!(!goto_line_is_valid("0, 0"));
        assert!(goto_line_is_valid("0, 0 mag 2^0"));
    }

    // r[verify cz.ui.coords-apply+1]
    // REQ-CTRL-APPLY / D-UI-1
    #[test]
    fn apply_enabled_when_valid_even_if_already_there() {
        assert!(apply_button_enabled(true, true));
    }

    // r[verify cz.ui.coords-apply+1]
    #[test]
    fn apply_enabled_when_valid_and_location_differs() {
        assert!(apply_button_enabled(true, false));
    }

    // r[verify cz.ui.coords-apply+1]
    #[test]
    fn apply_disabled_when_line_invalid_regardless_of_location() {
        assert!(!apply_button_enabled(false, false));
        assert!(!apply_button_enabled(false, true));
    }

    // B-GOTO-1: Accept whatever the location HUD produces.
    // r[verify cz.ui.goto-accepts-readout+1]
    #[test]
    fn goto_accepts_format_location_readout_roundtrip() {
        let loc = ul_for_center(IntExp::from(1), IntExp::from(-1), 5, TEST_SCREEN_RES);
        let (re, im) = viewport_center(&loc, TEST_SCREEN_RES);
        let text = format_location_readout(&re, &im, loc.zoom_pot);
        assert!(
            goto_line_is_valid(&text),
            "Apply must accept HUD readout; got invalid: {text}"
        );
        assert!(apply_button_enabled(goto_line_is_valid(&text), true));
        let cmds = commands_from_goto_line(&text).expect("commands");
        assert!(
            cmds.iter().any(|c| matches!(c, ZoomerCommand::SetZoom { pot: 5 })),
            "must apply mag 2^5 from readout"
        );
        assert!(
            matches!(cmds.first(), Some(ZoomerCommand::SetZoom { .. })),
            "SetZoom must precede SetPos so center uses target mag"
        );
    }

    // r[verify cz.ui.goto-accepts-readout+1]
    #[test]
    fn goto_accepts_pasted_hud_string_with_negative_imag_and_mag() {
        let line = "0.301025390625 -0.010498046875i mag 2^2";
        assert!(goto_line_is_valid(line), "screenshot-style paste must validate");
        let cmds = commands_from_goto_line(line).expect("commands");
        assert!(matches!(cmds.first(), Some(ZoomerCommand::SetZoom { pot: 2 })));
        match cmds.get(1) {
            Some(ZoomerCommand::SetPos { real, imag }) => {
                assert!((f64::from(real.clone()) - 0.301025390625).abs() < 1e-9);
                assert!((f64::from(imag.clone()) + 0.010498046875).abs() < 1e-9);
            }
            _ => panic!("expected SetPos after SetZoom"),
        }
    }

    // r[verify cz.ui.goto-accepts-readout+1]
    #[test]
    fn goto_accepts_plus_imag_form_and_legacy_triplet() {
        assert!(goto_line_is_valid("0.5 + 0.25i mag 2^0"));
        assert!(!goto_line_is_valid("0, 0"));
        assert!(goto_line_is_valid("1.5 -2 7"));
        let cmds = commands_from_goto_line("1.5 -2 7").expect("legacy");
        assert!(matches!(cmds.first(), Some(ZoomerCommand::SetZoom { pot: 7 })));
        assert!(matches!(cmds.get(1), Some(ZoomerCommand::SetPos { .. })));
    }

    // B-GOTO-2: same pasted location must land identically from different start mags.
    // r[verify cz.ui.goto-absolute-center+1]
    #[test]
    fn goto_apply_independent_of_starting_magnification() {
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::headgroup::window::sampling::SamplingContext;

        fn empty_ctx(zoom: i32) -> SamplingContext {
            SamplingContext {
                screen: None,
                screen_size: TEST_SCREEN_RES,
                location: ul_for_center(IntExp::ZERO, IntExp::ZERO, zoom, TEST_SCREEN_RES),
                updated: false,
                mouse_drag_start: None,
            }
        }

        let line = "0.301025390625 -0.010498046875i mag 2^2";
        let cmds = commands_from_goto_line(line).expect("valid");
        let mut from_mag5 = empty_ctx(5);
        let mut from_mag0 = empty_ctx(0);
        transform(cmds.clone(), &mut from_mag5);
        transform(cmds, &mut from_mag0);
        let (re5, im5) = viewport_center(&from_mag5.location, TEST_SCREEN_RES);
        let (re0, im0) = viewport_center(&from_mag0.location, TEST_SCREEN_RES);
        assert_eq!(from_mag5.location.zoom_pot, 2);
        assert_eq!(from_mag0.location.zoom_pot, 2);
        assert!(
            (f64::from(re5.clone()) - f64::from(re0.clone())).abs() < 1e-9,
            "re diverged across start mags"
        );
        assert!(
            (f64::from(im5.clone()) - f64::from(im0.clone())).abs() < 1e-9,
            "im diverged across start mags"
        );
        assert!((f64::from(re5) - 0.301025390625).abs() < 1e-9);
        assert!((f64::from(im0) + 0.010498046875).abs() < 1e-9);
    }

    // r[verify cz.ui.goto-absolute-center+1]
    #[test]
    fn goto_apply_roundtrip_after_pan_and_zoom() {
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::headgroup::window::sampling::SamplingContext;

        let screen = TEST_SCREEN_RES;
        let mut ctx = SamplingContext {
            screen: None,
            screen_size: screen,
            location: ul_for_center(
                f64_to_intexp(0.3),
                f64_to_intexp(-0.01),
                4,
                screen,
            ),
            updated: false,
            mouse_drag_start: None,
        };
        let (re0, im0) = viewport_center(&ctx.location, screen);
        let text = format_location_readout(&re0, &im0, ctx.location.zoom_pot);
        // Wander elsewhere.
        transform(
            vec![
                ZoomerCommand::SetZoom { pot: 1 },
                ZoomerCommand::SetPos {
                    real: IntExp::ZERO,
                    imag: IntExp::ZERO,
                },
            ],
            &mut ctx,
        );
        transform(commands_from_goto_line(&text).expect("roundtrip"), &mut ctx);
        let (re1, im1) = viewport_center(&ctx.location, screen);
        assert_eq!(ctx.location.zoom_pot, 4);
        assert!((f64::from(re1.clone()) - f64::from(re0.clone())).abs() < 1e-6);
        assert!((f64::from(im1) - f64::from(im0)).abs() < 1e-6);
    }

    // r[verify cz.ui.goto-absolute-center+1]
    #[test]
    fn goto_apply_from_different_pans_same_mag_agrees() {
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::headgroup::window::sampling::SamplingContext;

        let screen = TEST_SCREEN_RES;
        let mk = |re: f64, im: f64| SamplingContext {
            screen: None,
            screen_size: screen,
            location: ul_for_center(f64_to_intexp(re), f64_to_intexp(im), 3, screen),
            updated: false,
            mouse_drag_start: None,
        };
        let line = format_location_readout(
            &f64_to_intexp(0.25),
            &f64_to_intexp(0.1),
            6,
        );
        let cmds = commands_from_goto_line(&line).expect("valid");
        let mut a = mk(-2.0, 1.0);
        let mut b = mk(1.5, -0.5);
        transform(cmds.clone(), &mut a);
        transform(cmds, &mut b);
        let (ar, ai) = viewport_center(&a.location, screen);
        let (br, bi) = viewport_center(&b.location, screen);
        assert_eq!(a.location.zoom_pot, b.location.zoom_pot);
        assert!((f64::from(ar) - f64::from(br)).abs() < 1e-9);
        assert!((f64::from(ai) - f64::from(bi)).abs() < 1e-9);
    }

    // r[verify cz.ui.coords-parse+2]
    // REQ-CTRL-PARSE
    #[test]
    fn parse_braces_and_extra_spaces() {
        let (re, im) = parse_complex("{ 1 , 2 }").unwrap();
        assert!((f64::from(re) - 1.0).abs() < 1e-9);
        assert!((f64::from(im) - 2.0).abs() < 1e-9);
    }

    // r[verify cz.ui.coords-parse+2]
    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_complex("not a coordinate").is_none());
        assert!(parse_complex("1").is_none());
    }

    // r[verify cz.ui.coords-parse+2]
    #[test]
    fn goto_requires_magnification_except_home() {
        assert!(!goto_line_is_valid("1, 2"));
        assert!(!goto_line_is_valid("1 + 2i"));
        assert!(goto_line_is_valid("1 + 2i mag 2^3"));
        assert!(goto_line_is_valid("home"));
    }

    /// Headgroup property pin: arbitrary UTF-8 never panics the goto parsers.
    // r[verify cz.ui.coords-parse+2]
    proptest! {
        #[test]
        fn goto_parsers_never_panic_on_arbitrary_utf8(s in ".*") {
            let _ = goto_line_is_valid(&s);
            let _ = commands_from_goto_line(&s);
            let _ = parse_complex(&s);
        }
    }

    /// Headgroup property pin: HUD readout → goto recovers pot and lands near center.
    // r[verify cz.ui.location-readout+2]
    // r[verify cz.ui.goto-absolute-center+1]
    proptest! {
        #[test]
        fn readout_roundtrips_through_goto(
            re in -2.0f64..2.0,
            im in -2.0f64..2.0,
            pot in -4i32..24,
        ) {
            let screen = TEST_SCREEN_RES;
            let loc = ul_for_center(f64_to_intexp(re), f64_to_intexp(im), pot, screen);
            let (cre, cim) = viewport_center(&loc, screen);
            let line = format_location_readout(&cre, &cim, loc.zoom_pot);
            let cmds = commands_from_goto_line(&line)
                .unwrap_or_else(|| panic!("readout must be valid goto: {line}"));
            let mut ctx = SamplingContext {
                screen: None,
                screen_size: screen,
                location: ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, screen),
                updated: false,
                mouse_drag_start: None,
            };
            transform(cmds, &mut ctx);
            assert_eq!(ctx.location.zoom_pot, pot, "line={line}");
            let (got_re, got_im) = viewport_center(&ctx.location, screen);
            let err_re = (f64::from(got_re) - re).abs();
            let err_im = (f64::from(got_im) - im).abs();
            // Readout formatting is lossy at high |pot|; require coarse recovery.
            let tol = (2.0f64).powi((-pot).clamp(0, 20)) * 8.0 + 1e-3;
            assert!(
                err_re < tol && err_im < tol,
                "line={line} err=({err_re},{err_im}) tol={tol}"
            );
        }
    }

    /// Thought-killed pins for location math / Apply / f64→IntExp (location bar).
    #[test]
    fn mutant_kill_coords_location_and_apply() {
        // Apply ignores already-at-location (must stay `line_valid` only).
        assert!(apply_button_enabled(true, true));
        assert!(apply_button_enabled(true, false));
        assert!(!apply_button_enabled(false, true));
        assert!(!apply_button_enabled(false, false));
        assert_ne!(apply_button_enabled(false, true), true);

        // f64_to_intexp: zero short-circuit; sign; normalize loops (*2 /÷2).
        assert_eq!(f64_to_intexp(0.0), IntExp::ZERO);
        assert_eq!(f64_to_intexp(-0.0), IntExp::ZERO);
        let four = f64_to_intexp(4.0);
        assert!((f64::from(four.clone()) - 4.0).abs() < 1e-12);
        let quarter = f64_to_intexp(0.25);
        assert!((f64::from(quarter.clone()) - 0.25).abs() < 1e-12);
        assert!((f64::from(f64_to_intexp(-2.5)) + 2.5).abs() < 1e-12);
        assert_ne!(f64::from(four), 0.0);

        // viewport_center ∘ ul_for_center ≈ identity on math center.
        let screen = (40u32, 71u32);
        let cre = f64_to_intexp(-0.75);
        let cim = f64_to_intexp(0.125);
        let ul = ul_for_center(cre.clone(), cim.clone(), -2, screen);
        let (r2, i2) = viewport_center(&ul, screen);
        assert!((f64::from(r2) - f64::from(cre.clone())).abs() < 1e-9);
        assert!((f64::from(i2.clone()) - f64::from(cim.clone())).abs() < 1e-9);
        // Wrong imag sign on UL would invert center imag.
        assert_ne!(f64::from(i2), -0.125);

        let line = format_location_readout(&cre, &cim, -2);
        assert!(line.contains("mag 2^-2"));
        assert!(line.contains('i'));
        assert_ne!(format_intexp_readout(&IntExp::ZERO), "nan");
        assert_eq!(format_intexp_readout(&IntExp::ZERO), "0");

        // Negative imag uses leading '-' (not "+ -…i").
        let neg_im = format_location_readout(&cre, &f64_to_intexp(-0.5), 3);
        assert!(neg_im.contains('-'));
        assert!(!neg_im.contains("+ -"));
        assert!(neg_im.contains("mag 2^3"));

        // parse_complex: comma pair, a+bi, bare i, reject real-only.
        let (re, im) = parse_complex("1.5,-2.25").expect("comma pair");
        assert!((f64::from(re) - 1.5).abs() < 1e-9);
        assert!((f64::from(im) + 2.25).abs() < 1e-9);
        let (re, im) = parse_complex("0.75+1.25i").expect("a+bi");
        assert!((f64::from(re) - 0.75).abs() < 1e-9);
        assert!((f64::from(im) - 1.25).abs() < 1e-9);
        let (re, im) = parse_complex("i").expect("bare i");
        assert_eq!(re, IntExp::ZERO);
        assert_eq!(im, IntExp::from(1));
        let (re, im) = parse_complex("-i").expect("-i");
        assert_eq!(re, IntExp::ZERO);
        assert_eq!(im, IntExp::from(-1));
        assert!(parse_complex("1.5").is_none());
        assert!(parse_complex("").is_none());
        assert!(parse_complex("   ").is_none());

        // Goto: SetZoom before SetPos; home; mag suffix; invalid without pot.
        let cmds = commands_from_goto_line("-0.75 + 0.125i  mag 2^-2").expect("hud goto");
        assert_eq!(cmds.len(), 2);
        match &cmds[0] {
            ZoomerCommand::SetZoom { pot } => assert_eq!(*pot, -2),
            _ => panic!("first must be SetZoom"),
        }
        match &cmds[1] {
            ZoomerCommand::SetPos { real, imag } => {
                assert!((f64::from(real.clone()) + 0.75).abs() < 1e-9);
                assert!((f64::from(imag.clone()) - 0.125).abs() < 1e-9);
            }
            _ => panic!("second must be SetPos"),
        }
        assert!(goto_line_is_valid("home"));
        let home = commands_from_goto_line("HOME").expect("home");
        assert!(matches!(home[0], ZoomerCommand::MoveTo { .. }));
        assert!(matches!(home[1], ZoomerCommand::SetZoom { .. }));
        assert!(!goto_line_is_valid(""));
        assert!(
            !goto_line_is_valid("1+2i"),
            "complex without mag must not be a valid goto"
        );
        // "mag" inside "imaginary" must not count as a magnification suffix.
        assert!(!goto_line_is_valid("1+2imaginary"));
        assert!(goto_line_is_valid("1 2 -3")); // legacy triple
    }

    /// Thought-killed pins: mag token/`2**`, word-boundary split, readout thresholds.
    #[test]
    fn mutant_kill_parse_mag_and_format_readout() {
        assert_eq!(parse_mag_token("2^-2"), Some(-2));
        assert_eq!(parse_mag_token("2^3"), Some(3));
        assert_eq!(parse_mag_token("2**10"), Some(10));
        assert_eq!(parse_mag_token("2**0"), Some(0));
        assert!(parse_mag_token("2^").is_none());
        assert!(parse_mag_token("3^2").is_none());
        assert!(parse_mag_token("").is_none());
        assert!(parse_mag_token("mag 2^1").is_none()); // needs stripped prefix
        // Dropping 2** branch would reject `2**N`.
        assert_ne!(parse_mag_token("2**5"), None);

        let (body, pot) = split_mag_suffix("1+2i  mag 2^-2");
        assert_eq!(pot, Some(-2));
        assert!(body.contains('i'));
        assert!(!body.to_lowercase().contains("mag"));
        let (body2, pot2) = split_mag_suffix("-0.75 + 0.125i mag 2**4");
        assert_eq!(pot2, Some(4));
        assert!(body2.contains("0.75"));
        // Word-boundary: "imaginary" must not strip as mag.
        let (body3, pot3) = split_mag_suffix("1+2imaginary");
        assert!(pot3.is_none());
        assert_eq!(body3, "1+2imaginary");
        assert!(split_mag_suffix("nimag 2^1").1.is_none());
        assert_eq!(split_mag_suffix("mag 2^7").1, Some(7));

        assert_eq!(format_intexp_readout(&IntExp::ZERO), "0");
        assert_eq!(format_intexp_readout(&f64_to_intexp(1.5)), "1.5");
        // Sci thresholds: |x|>=1e6 or |x|<1e-4.
        let big = format_intexp_readout(&f64_to_intexp(1e6));
        assert!(big.contains('e') || big.contains('E'), "got {big}");
        let tiny = format_intexp_readout(&f64_to_intexp(1e-5));
        assert!(tiny.contains('e') || tiny.contains('E'), "got {tiny}");
        let mid = format_intexp_readout(&f64_to_intexp(0.001));
        assert!(!mid.contains('e') && !mid.contains('E'), "got {mid}");
        // Trim trailing zeros / trailing dot.
        assert_eq!(format_intexp_readout(&f64_to_intexp(2.0)), "2");
        assert_ne!(format_intexp_readout(&f64_to_intexp(2.0)), "2.000000000000");

        let d = decimal_str_to_intexp(" -1.25 ").expect("parse");
        assert!((f64::from(d) + 1.25).abs() < 1e-9);
        assert!(decimal_str_to_intexp("not-a-number").is_none());
        assert!(decimal_str_to_intexp("").is_none());
    }
}
