use crate::settings::*;
use crate::assemblies::shadergroup::escaper::*;
use crate::utils::*;
use std::f64::consts::*;
use std::time::*;

use egui::Color32;
pub fn color(values: &ZoomerValuesScreen, settings:&mut Settings) -> Vec<Color32> {
    let mut returned = vec!((0,0,0);(values.res.0*values.res.1) as usize);
    let res = values.res;
    if let Some(instructions) = &mut settings.coloring_script {
        for instruction in instructions {
            match instruction {
                ColoringInstruction::PaintEscapeTime{
                    opacity, color, range, shading_method, normalizing_method, ..
                } => {
                    let start = Instant::now();
                    let period = shading_method.period.determine();
                    let period_recip = 1.0/period;
                    let phase = shading_method.phase.determine();

                    let range = *range as f64 / 255.0;

                    let shade =
                        match shading_method.shading {
                            Shading::Modular{..} => {
                                |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
                                    ((n+phase) % period)*period_recip
                                }
                            }
                            Shading::Sinus{..} => {
                                |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
                                    (1.0-((n+phase)*TAU*period_recip).cos())*0.5
                                }
                            }
                        };

                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            let color = match value {
                                ScreenValue::Inside{..} => {continue;}
                                ScreenValue::Outside{big_time: escape_time, ..} => {
                                    let escape_time = *escape_time as f64;
                                    let escape_time = normalizing_method.normalize(&escape_time);
                                    let brightness = match shading_method.shading {
                                        Shading::Modular{..} => {
                                            ((escape_time+phase) % period)*period_recip
                                        }
                                        Shading::Sinus{..} => {
                                            (1.0-((escape_time+phase)*TAU*period_recip).cos())*0.5
                                        }
                                    };
                                    let color = modify_color(*color, brightness, range);
                                    (color.0,color.1,color.2,*opacity)
                                }
                            };
                            returned[index]= layer_colors(returned[index], color)

                        }
                    }
                    //println!("painted escape time in {:6}", start.elapsed().as_secs_f64())
                }
                ColoringInstruction::PaintSmallTime{
                    inside_opacity, outside_opacity, color, range, shading_method, normalizing_method, ..
                } => {
                    let start = Instant::now();
                    let period = shading_method.period.determine();
                    let period_recip = 1.0/period;
                    let phase = shading_method.phase.determine();

                    let range = *range as f64 / 255.0;

                    let shade =
                        match shading_method.shading {
                            Shading::Modular{..} => {
                                |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
                                    ((n+phase) % period)*period_recip
                                }
                            }
                            Shading::Sinus{..} => {
                                |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
                                    (1.0-((n+phase)*TAU*period_recip).cos())*0.5
                                }
                            }
                        };

                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            let (smalltime, opacity) = match value {
                                ScreenValue::Inside{small_time, ..} => {
                                    (small_time, &inside_opacity)
                                }
                                ScreenValue::Outside{small_time, ..} => {
                                    (small_time, &outside_opacity)
                                }
                            };

                            let color = {
                                let smalltime = *smalltime as f64;
                                let smalltime = normalizing_method.normalize(&smalltime);
                                let brightness = shade(&phase, &period, &period_recip, &smalltime);
                                let color = modify_color(*color, brightness, range);
                                (color.0,color.1,color.2,**opacity)
                            };
                            returned[index]=layer_colors(returned[index], color)
                        }
                    }
                    //println!("painted small time in {:6}", start.elapsed().as_secs_f64())
                }
                ColoringInstruction::PaintSmallness{
                    inside_opacity, outside_opacity, color, range, shading_method, normalizing_method, ..
                } => {
                    let start = Instant::now();
                    let period = shading_method.period.determine();
                    let period_recip = 1.0/period;
                    let phase = shading_method.phase.determine();

                    let range = *range as f64 / 255.0;

                    let shade =
                        match shading_method.shading {
                            Shading::Modular{..} => {
                                |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
                                    ((n+phase) % period)*period_recip
                                }
                            }
                            Shading::Sinus{..} => {
                                |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
                                    (1.0-((n+phase)*TAU*period_recip).cos())*0.5
                                }
                            }
                        };

                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            let (smallness, opacity) = match value {
                                ScreenValue::Inside{smallness, ..} => {
                                    (smallness, &inside_opacity)
                                }
                                ScreenValue::Outside{smallness, ..} => {
                                    (smallness, &outside_opacity)
                                }
                            };

                            let color = {
                                let smallness = *smallness as f64;
                                let smallness = normalizing_method.normalize(&smallness);
                                let brightness = shade(&phase, &period, &period_recip, &smallness);
                                let color = modify_color(*color, brightness, range);
                                (color.0,color.1,color.2,**opacity)
                            };
                            returned[index]=layer_colors(returned[index], color)
                        }
                    }
                    //println!("painted smallness in {:6}", start.elapsed().as_secs_f64())
                }
                ColoringInstruction::HighlightInFilaments{
                    opacity, color, ..
                } => {
                    let start = Instant::now();
                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            match value {
                                ScreenValue::Inside{..} => {continue;}
                                ScreenValue::Outside{..} => {
                                    let in_filament = is_in_filament(&values, pos);
                                    if in_filament {
                                        let color = (
                                            color.0
                                            , color.1
                                            , color.2
                                            , *opacity
                                            );
                                        returned[index]=layer_colors(returned[index], color)
                                    }
                                }
                            }
                        }
                    }
                    //println!("highlighted in filaments in {:6}", start.elapsed().as_secs_f64())
                }
                ColoringInstruction::HighlightOutFilaments{
                    opacity, color, ..
                } => {
                    let start = Instant::now();
                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            match value {
                                ScreenValue::Inside{..} => {
                                    let out_filament = is_out_filament(values, pos);
                                    if out_filament {
                                        let color = (
                                            color.0
                                            , color.1
                                            , color.2
                                            , *opacity
                                        );
                                        returned[index]=layer_colors(returned[index], color)
                                    }
                                }
                                ScreenValue::Outside{..} => {continue;}
                            }
                        }
                    }
                    //println!("highlighted out filaments in {:6}", start.elapsed().as_secs_f64())
                }
                ColoringInstruction::HighlightNodes{
                    inside_opacity, outside_opacity, color, thickness, ..
                } => {
                    let start = Instant::now();
                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            let (is_node, opacity) = match value {
                                ScreenValue::Inside{..} => {
                                    let node = is_node(values, pos, *thickness);
                                    (node, &inside_opacity)
                                }
                                ScreenValue::Outside{..} => {
                                    let node = is_node(values, pos, *thickness);
                                    (node, &outside_opacity)
                                }
                            };
                            if is_node {
                                let color = (
                                    color.0
                                    , color.1
                                    , color.2
                                    , **opacity
                                );
                                returned[index]=layer_colors(returned[index], color)
                            }
                        }
                    }
                    //println!("highlighted nodes in {:6}", start.elapsed().as_secs_f64())
                }
                ColoringInstruction::HighlightSmallTimeEdges{
                    inside_opacity, outside_opacity, color, ..
                } => {
                    let start = Instant::now();
                    use std::cmp::*;
                    for x in 0..res.0 {
                        for y in 0..res.1 {
                            let pos = (x as i32, y as i32);
                            let index = index_from_pos(&pos, res.0);
                            let value = &values.values[index];
                            let (is_edge, opacity) = match value {
                                ScreenValue::Inside{..} => {
                                    let edge = is_node_tree(values, pos);
                                    (edge, &inside_opacity)
                                }
                                ScreenValue::Outside{..} => {
                                    let edge = is_node_tree(values, pos);
                                    (edge, &outside_opacity)
                                }
                            };
                            if is_edge {
                                let color = (
                                    color.0
                                    , color.1
                                    , color.2
                                    , **opacity
                                );
                                returned[index]=layer_colors(returned[index], color)
                            }
                        }
                    }
                    //println!("highlighted node tree in {:6}", start.elapsed().as_secs_f64())
                }
            }
        }
    }
    let mut returned_color32 = vec!();
    for c in returned {
        returned_color32.push(
            Color32::from_rgb(c.0,c.1,c.2)
        )
    }
    returned_color32
}

pub fn layer_colors (bottom: (u8, u8, u8), top:(u8, u8, u8, u8)) -> (u8, u8, u8) {
    let top_share = top.3;
    let bottom_share = 255-top_share;
    (
        ((bottom.0 as u32 * bottom_share as u32 + top.0 as u32 * top_share as u32)>>8) as u8
        , ((bottom.1 as u32 * bottom_share as u32 + top.1 as u32 * top_share as u32)>>8) as u8
        , ((bottom.2 as u32 * bottom_share as u32 + top.2 as u32 * top_share as u32)>>8) as u8
    )
}

use std::cmp::*;
pub fn modify_color (color:(u8, u8, u8), brightness: f64, range:f64) -> (u8, u8, u8) {
let mut delta_b = (((brightness*255.0)-127.0) * range) as i32;
let color_max = max(max(color.0, color.1), color.2) as i32;
let color_min = min(min(color.0, color.1), color.2) as i32;
if color_min + delta_b < 0 {delta_b = 0-color_min}
if color_max + delta_b > 255 {delta_b = 255-color_max}
    (
        (color.0 as i32 +delta_b) as u8
        , (color.1 as i32+delta_b) as u8
        , (color.2 as i32+delta_b) as u8
    )
}

pub fn is_in_filament(values: &ZoomerValuesScreen, pos: (i32, i32)) -> bool {
    // r[impl cz.craft.screen-space-derivative-edges+1]
    let points = [
        pos
        , (pos.0, pos.1-1) // up
        , (pos.0, pos.1+1) // down
        , (pos.0-1, pos.1) // left
        , (pos.0+1, pos.1) // right
    ];

    // Each remapped sample describes a locally linear field at its source.
    // Project that field to the center screen pixel before looking for a peak.
    let sample = |sample_pos: (i32, i32)| -> Option<(f64, f64, f32)> {
        match safe_sample(&values.values, sample_pos, values.res)? {
            ScreenValue::Outside { big_time, gradient_angle, .. } => {
                let offset = (
                    (pos.0 - sample_pos.0) as f64,
                    (pos.1 - sample_pos.1) as f64,
                );
                let projection = offset.0 * (*gradient_angle as f64).cos()
                    + offset.1 * (*gradient_angle as f64).sin();
                Some((*big_time as f64, *big_time as f64 + projection, *gradient_angle))
            }
            ScreenValue::Inside { .. } => None,
        }
    };
    let samples = points.map(sample);

    // Axis peak with two layers of honesty:
    // 1. Extrapolated peak keeps thin screen-space ridges past remap.
    // 2. Raw escape times on that axis must not be a flat plateau. The old
    //    integer test was deaf to equal-n neighborhoods; conjugation-symmetric
    //    exterior rays (cusp / bulb-axis tendrils) live in those plateaus and
    //    must stay dark. A true filament has a raw escape-time contrast.
    let axis_peak = |a: usize, b: usize| -> bool {
        let (Some((c_raw, c_ext, _)), Some((a_raw, a_ext, _)), Some((b_raw, b_ext, _))) =
            (samples[0], samples[a], samples[b])
        else {
            return false;
        };
        let extrapolated_peak = c_ext > a_ext && c_ext > b_ext;
        // Flat (±0) or near-flat (±1) raw neighborhoods are not filaments —
        // boundary speckles / tendril edges live in those bands.
        let raw_contrast = (c_raw - a_raw).abs() > 1.0 || (c_raw - b_raw).abs() > 1.0;
        extrapolated_peak && raw_contrast
    };

    axis_peak(1, 2) || axis_peak(3, 4)
}


pub fn is_out_filament(values: &ZoomerValuesScreen, pos: (i32, i32)) -> bool {

    let points = [
        pos
        , (pos.0, pos.1-1) // up
        , (pos.0, pos.1+1) // down
        , (pos.0-1, pos.1) // left
        , (pos.0+1, pos.1) // right
    ];

    let p_values = [
        get_loop_period(safe_sample(&values.values, points[0], values.res))
        ,get_loop_period(safe_sample(&values.values, points[1], values.res))
        ,get_loop_period(safe_sample(&values.values, points[2], values.res))
        ,get_loop_period(safe_sample(&values.values, points[3], values.res))
        ,get_loop_period(safe_sample(&values.values, points[4], values.res))
    ];

    /*let s_values = [
        get_smallness(safe_sample(&values.values, points[0], values.res))
        ,get_smallness(safe_sample(&values.values, points[1], values.res))
        ,get_smallness(safe_sample(&values.values, points[2], values.res))
        ,get_smallness(safe_sample(&values.values, points[3], values.res))
        ,get_smallness(safe_sample(&values.values, points[4], values.res))
    ];*/

    is_increased(
        p_values[0], p_values[1], p_values[2], p_values[3], p_values[4]
    )/* && is_decreased(
        s_values[0], s_values[1], s_values[2], s_values[3], s_values[4]
    )*/
}



pub fn is_node_tree(values: &ZoomerValuesScreen, pos: (i32, i32)) -> bool {

    let points = [
        pos
        , (pos.0, pos.1-1) // up
        , (pos.0, pos.1+1) // down
        , (pos.0-1, pos.1) // left
        , (pos.0+1, pos.1) // right
    ];

    let values = [
        get_small_time(safe_sample(&values.values, points[0], values.res))
        ,get_small_time(safe_sample(&values.values, points[1], values.res))
        ,get_small_time(safe_sample(&values.values, points[2], values.res))
        ,get_small_time(safe_sample(&values.values, points[3], values.res))
        ,get_small_time(safe_sample(&values.values, points[4], values.res))
    ];

    is_increased(
        values[0], values[1], values[2], values[3], values[4]
    )
}

pub fn is_node(values: &ZoomerValuesScreen, pos: (i32, i32), thickness: u8) -> bool {

    let points = [
        pos
        , (pos.0, pos.1-thickness as i32) // up
        , (pos.0, pos.1+thickness as i32) // down
        , (pos.0-thickness as i32, pos.1) // left
        , (pos.0+thickness as i32, pos.1) // right
    ];

    let s_values = [
        get_smallness(safe_sample(&values.values, points[0], values.res))
        ,get_smallness(safe_sample(&values.values, points[1], values.res))
        ,get_smallness(safe_sample(&values.values, points[2], values.res))
        ,get_smallness(safe_sample(&values.values, points[3], values.res))
        ,get_smallness(safe_sample(&values.values, points[4], values.res))
    ];

    is_local_minimum(
        s_values[0], s_values[1], s_values[2], s_values[3], s_values[4]
    )// && is_node_tree(values, pos)
}

pub fn is_increased<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {
    if let (Some(value), Some(up)) = (&value, up) {
        if up < *value {
            return true
        }
    }
    if let (Some(value), Some(down)) = (&value, down) {
        if down < *value {
            return true
        }
    }
    if let (Some(value), Some(left)) = (&value, left) {
        if left < *value {
            return true
        }
    }
    if let (Some(value), Some(right)) = (&value, right) {
        if right < *value {
            return true
        }
    }
    false
}

pub fn is_decreased<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {
    if let (Some(value), Some(up)) = (&value, up) {
        if up > *value {
            return true
        }
    }
    if let (Some(value), Some(down)) = (&value, down) {
        if down > *value {
            return true
        }
    }
    if let (Some(value), Some(left)) = (&value, left) {
        if left > *value {
            return true
        }
    }
    if let (Some(value), Some(right)) = (&value, right) {
        if right > *value {
            return true
        }
    }
    false
}

pub fn is_changed<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {
    if let (Some(value), Some(up)) = (&value, up) {
        if up != *value {
            return true
        }
    }
    if let (Some(value), Some(down)) = (&value, down) {
        if down != *value {
            return true
        }
    }
    if let (Some(value), Some(left)) = (&value, left) {
        if left != *value {
            return true
        }
    }
    if let (Some(value), Some(right)) = (&value, right) {
        if right != *value {
            return true
        }
    }
    false
}


pub fn slope_sign_changed<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {

    if let (Some(value), Some(up), Some(down)) = (&value, up, down) {
        if down < *value && *value > up {
            return true
        }
    }

    if let (Some(value), Some(left), Some(right)) = (value, left, right) {
        if left < value && value > right {
            return true
        }
    }

    false
}

pub fn is_local_minimum<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {

    if let (Some(value), Some(up), Some(down), Some(left), Some(right)) = (&value, up, down, left, right) {
        if down > *value && *value < up
        && left > *value && *value < right {
            return true
        }
    }

    false
}

pub fn get_loop_period(value: Option<&ScreenValue>) -> Option<u32> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{..} => {return None}
            ScreenValue::Inside{loop_period, ..} => {
                // Period 0 is "interior, period unknown", not a numeric period.
                // Unknown values must not create filament edges.
                return (*loop_period != 0).then_some(*loop_period)
            }
        }
    } else {None}

}

pub fn get_escape_time(value: Option<&ScreenValue>) -> Option<u32> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{big_time, ..} => {return Some(*big_time)}
            ScreenValue::Inside{..} => {return None }
        }
    } else {None}

}

pub fn get_small_time(value: Option<&ScreenValue>) -> Option<u32> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{small_time, ..} => {return Some(*small_time)}
            ScreenValue::Inside{small_time, ..} => {return Some(*small_time)}
        }
    } else {None}

}

pub fn get_smallness(value: Option<&ScreenValue>) -> Option<f64> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{smallness, ..} => {return Some(*smallness)}
            ScreenValue::Inside{smallness, ..} => {return Some(*smallness)}
        }
    } else {None}

}



use std::ops::*;
pub fn safe_sample<T: Index<usize, Output=J>, J>(stuff:&T, pos:(i32, i32), res:(u32, u32)) -> Option<&J> {
    if let Some(i) = index_from_pos_safe(&pos, res) {Some(&stuff[i])} else {None}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::shadergroup::escaper::{ScreenValue, ZoomerValuesScreen};
    use crate::utils::ObjectivePosAndZoom;

    fn inside(period: u32) -> ScreenValue {
        ScreenValue::Inside { small_time: 0, loop_period: period, smallness: 0.0 }
    }

    fn outside(time: u32, angle: f32) -> ScreenValue {
        ScreenValue::Outside {
            big_time: time,
            small_time: 0,
            smallness: 0.0,
            gradient_angle: angle,
        }
    }

    fn screen(values: Vec<ScreenValue>) -> ZoomerValuesScreen {
        ZoomerValuesScreen {
            values,
            res: (3, 3),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 0,
            },
        hud: Default::default()
    }
    }

    // verifies r[cz.craft.period-derivative-test+1]
    #[test]
    fn unknown_period_never_creates_out_filament() {
        let center_verified = screen(vec![
            inside(0), inside(2), inside(0),
            inside(2), inside(2), inside(2),
            inside(0), inside(2), inside(0),
        ]);
        assert!(!is_out_filament(&center_verified, (1, 1)),
            "unknown neighboring periods must be ignored");

        let center_unknown = screen(vec![
            inside(2), inside(0), inside(2),
            inside(0), inside(0), inside(0),
            inside(2), inside(0), inside(2),
        ]);
        assert!(!is_out_filament(&center_unknown, (1, 1)),
            "an unknown center must not light itself");
    }

    // verifies r[cz.craft.period-derivative-test+1]
    #[test]
    fn differing_verified_periods_still_create_out_filament() {
        let values = screen(vec![
            inside(1), inside(1), inside(1),
            inside(1), inside(2), inside(1),
            inside(1), inside(1), inside(1),
        ]);
        assert!(is_out_filament(&values, (1, 1)),
            "real period boundaries must remain visible");
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn caught_up_view_matches_old_raw_peak_oracle() {
        // When the worker has caught up, every screen neighbor is a distinct
        // data pixel. The old look is the strict raw escape-time peak. With
        // zero angles, extrapolation is a no-op on the vertical axis and must
        // reproduce that oracle exactly — including the dark cells.
        let times = [
            4u32, 8, 4,
            4, 8, 4,
            3, 5, 3,
        ];
        let values = ZoomerValuesScreen {
            values: times.into_iter().map(|t| outside(t, 0.0)).collect(),
            res: (3, 3),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 0,
            },
        hud: Default::default()
    };
        for y in 0..3 {
            for x in 0..3 {
                let pos = (x, y);
                let old = slope_sign_changed(
                    get_escape_time(safe_sample(&values.values, pos, values.res)),
                    get_escape_time(safe_sample(&values.values, (pos.0, pos.1 - 1), values.res)),
                    get_escape_time(safe_sample(&values.values, (pos.0, pos.1 + 1), values.res)),
                    get_escape_time(safe_sample(&values.values, (pos.0 - 1, pos.1), values.res)),
                    get_escape_time(safe_sample(&values.values, (pos.0 + 1, pos.1), values.res)),
                );
                assert_eq!(is_in_filament(&values, pos), old,
                    "caught-up parity broken at ({x},{y})");
            }
        }
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn conjugation_axis_tendril_stays_dark() {
        // Home-view false filaments: a flat escape-time band with conjugation
        // symmetry. Opposite flanks point away from the axis, so naive
        // extrapolation manufactures a peak the old integer test never saw.
        // The whole horizontal mid-row must stay dark.
        let mut samples = Vec::new();
        for y in 0..5 {
            for _x in 0..9 {
                // Angles point away from the mid-row (y=2): up-flank down? 
                // Away from axis: y<2 points up (-π/2 in screen y-down), y>2 points down.
                let angle = if y < 2 {
                    -FRAC_PI_2 as f32
                } else if y > 2 {
                    FRAC_PI_2 as f32
                } else {
                    0.0
                };
                samples.push(outside(20, angle));
            }
        }
        let values = ZoomerValuesScreen {
            values: samples,
            res: (9, 5),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 0,
            },
        hud: Default::default()
    };
        for x in 1..8 {
            assert!(!is_in_filament(&values, (x, 2)),
                "conjugation-axis tendril lit at ({x}, 2)");
        }
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn monotone_exterior_field_never_lights() {
        // Smooth outside-set field: escape time falls to the right and every
        // gradient points right (increasing |z|). No screen pixel is a ridge.
        let mut samples = Vec::new();
        for _y in 0..5 {
            for x in 0..7 {
                samples.push(outside(40 - x as u32, 0.0));
            }
        }
        let values = ZoomerValuesScreen {
            values: samples,
            res: (7, 5),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 2,
            },
        hud: Default::default()
    };
        for y in 1..4 {
            for x in 1..6 {
                assert!(!is_in_filament(&values, (x, y)),
                    "monotone exterior field lit a false filament at ({x}, {y})");
            }
        }
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn near_flat_escape_delta_one_stays_dark() {
        // Boundary speckles: escape time only differs by one from neighbors.
        // That is still a plateau for filament purposes — must stay dark.
        let times = [
            5u32, 5, 5,
            5, 6, 5,
            5, 5, 5,
        ];
        let values = ZoomerValuesScreen {
            values: times.into_iter().map(|t| outside(t, FRAC_PI_2 as f32)).collect(),
            res: (3, 3),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 0,
            },
        hud: Default::default()
    };
        assert!(!is_in_filament(&values, (1, 1)),
            "±1 escape-time bump must not light as a filament");
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn true_ridge_stays_one_pixel_with_raw_contrast() {
        // A real in-filament has an elevated escape-time spine. It must light
        // exactly one screen column — the thin line the old look had — not a
        // block of neighbors.
        for width in [8i32, 16] {
            let ridge = width / 2;
            let mut samples = Vec::new();
            for _y in 0..3 {
                for x in 0..width {
                    let (time, angle) = if x < ridge {
                        (4, PI as f32)
                    } else if x > ridge {
                        (4, 0.0)
                    } else {
                        (8, FRAC_PI_2 as f32)
                    };
                    samples.push(outside(time, angle));
                }
            }
            let values = ZoomerValuesScreen {
                values: samples,
                res: (width as u32, 3),
                objective_location: ObjectivePosAndZoom {
                    pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                    zoom_pot: if width == 8 { 1 } else { 2 },
                },
                            hud: Default::default()
};
            let lit: Vec<i32> = (1..width - 1)
                .filter(|x| is_in_filament(&values, (*x, 1)))
                .collect();
            assert_eq!(lit, vec![ridge], "true ridge must stay exactly one screen pixel");
        }
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn remapped_duplicate_block_does_not_become_a_thick_band() {
        // After a 2x remap a 1px ridge becomes a 2-wide block of identical
        // elevated answers. Lighting every raw boundary of that block makes
        // the "flashing big blocks" regression. At most one column may light;
        // lighting none is also acceptable (interim honesty) — never two.
        let mut samples = Vec::new();
        // Cross-section: 4,4,8,8,4,4 — duplicated ridge block.
        let row = [4u32, 4, 8, 8, 4, 4];
        for _y in 0..3 {
            for (x, &t) in row.iter().enumerate() {
                let angle = if (x as i32) < 2 {
                    PI as f32
                } else if (x as i32) > 3 {
                    0.0
                } else {
                    FRAC_PI_2 as f32
                };
                samples.push(outside(t, angle));
            }
        }
        let values = ZoomerValuesScreen {
            values: samples,
            res: (6, 3),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 1,
            },
        hud: Default::default()
    };
        let lit: Vec<i32> = (1..5)
            .filter(|x| is_in_filament(&values, (*x, 1)))
            .collect();
        assert!(lit.len() <= 1,
            "remapped ridge block thickened into {:?}; want at most one column", lit);
    }

    /// Thought-killed pins for color blend / neighborhood predicates / safe_sample.
    /// Shadergroup colorer (charter note in issue-stack): pure helpers only.
    #[test]
    fn mutant_kill_colorer_blend_and_neighborhood() {
        // Opaque top replaces bottom; alpha 0 keeps bottom; mid blends via >>8.
        assert_eq!(
            layer_colors((10, 20, 30), (200, 100, 50, 255)),
            (
                ((10u32 * 0 + 200u32 * 255) >> 8) as u8,
                ((20u32 * 0 + 100u32 * 255) >> 8) as u8,
                ((30u32 * 0 + 50u32 * 255) >> 8) as u8,
            )
        );
        // Alpha 0: bottom weighted by 255, then >>8 (not identity — kills >>→<< / *→+).
        let keep = layer_colors((10, 20, 30), (0, 0, 0, 0));
        assert_eq!(
            keep,
            (
                ((10u32 * 255) >> 8) as u8,
                ((20u32 * 255) >> 8) as u8,
                ((30u32 * 255) >> 8) as u8,
            )
        );
        assert_ne!(keep, (10, 20, 30)); // exact identity would miss the >>8 path
        let mid = layer_colors((0, 0, 0), (255, 0, 0, 128));
        assert_ne!(mid, (0, 0, 0));
        assert_ne!(mid.0, 255); // not full replace at alpha 128
        assert!(mid.0 < 200);

        let bright = modify_color((100, 100, 100), 1.0, 1.0);
        assert!(bright.0 > 100);
        let dark = modify_color((100, 100, 100), 0.0, 1.0);
        assert!(dark.0 < 100);
        // Clamp: cannot push past 0/255.
        assert_eq!(modify_color((0, 0, 0), 0.0, 1.0), (0, 0, 0));
        assert_eq!(modify_color((255, 255, 255), 1.0, 1.0), (255, 255, 255));

        assert!(is_increased(Some(5), Some(3), None, None, None));
        assert!(!is_increased(Some(5), Some(5), Some(6), Some(7), Some(8)));
        assert!(is_decreased(Some(5), Some(7), None, None, None));
        assert!(!is_decreased(Some(5), Some(5), Some(4), Some(3), Some(2)));
        assert!(is_local_minimum(Some(1), Some(2), Some(3), Some(4), Some(5)));
        assert!(!is_local_minimum(Some(5), Some(2), Some(3), Some(4), Some(1)));
        // Missing neighbor → false (not || short-circuit true).
        assert!(!is_local_minimum(Some(1), Some(2), Some(3), Some(4), None));

        let buf = vec![1u8, 2, 3, 4];
        assert_eq!(safe_sample(&buf, (0, 0), (2, 2)), Some(&1));
        assert_eq!(safe_sample(&buf, (1, 0), (2, 2)), Some(&2));
        assert_eq!(safe_sample(&buf, (0, 1), (2, 2)), Some(&3));
        assert_eq!(safe_sample(&buf, (1, 1), (2, 2)), Some(&4));
        assert_eq!(safe_sample(&buf, (-1, 0), (2, 2)), None);
        assert_eq!(safe_sample(&buf, (2, 0), (2, 2)), None);

        assert_eq!(get_loop_period(Some(&inside(0))), None);
        assert_eq!(get_loop_period(Some(&inside(3))), Some(3));
        assert_eq!(get_escape_time(Some(&outside(9, 0.0))), Some(9));
        assert_eq!(get_escape_time(Some(&inside(1))), None);

        // is_changed / slope_sign_changed: != and peak-on-axis (not flat).
        assert!(is_changed(Some(5), Some(6), None, None, None));
        assert!(!is_changed(Some(5), Some(5), Some(5), Some(5), Some(5)));
        assert!(!is_changed(Some(5), None, None, None, None));
        assert!(slope_sign_changed(Some(5), Some(3), Some(3), None, None)); // up/down both < → peak
        assert!(!slope_sign_changed(Some(5), Some(6), Some(7), None, None)); // monotone
        assert!(!slope_sign_changed(Some(5), Some(3), None, None, None)); // needs both axis ends
        // Horizontal peak: left/right both below.
        assert!(slope_sign_changed(Some(5), None, None, Some(2), Some(2)));
        // is_increased/decreased on left/right axes (not only up).
        assert!(is_increased(Some(5), None, None, Some(3), None));
        assert!(is_decreased(Some(5), None, None, None, Some(8)));
        assert_eq!(get_small_time(Some(&outside(1, 0.25))), Some(0));
        assert_eq!(get_small_time(Some(&inside(1))), Some(0));
        assert_eq!(get_smallness(Some(&inside(1))), Some(0.0));
        assert_eq!(get_smallness(None), None);
        assert_eq!(get_escape_time(None), None);
    }
}
