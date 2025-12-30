use crate::action::settings::*;
use crate::actor::escaper::*;
use crate::action::utils::*;
use std::f64::consts::*;
use std::time::*;
pub(crate) fn color(values: &ZoomerValuesScreen, settings:&mut Settings) -> Vec<(u8, u8, u8)> {
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
                            returned[index]=layer_colors(returned[index], color)
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
    returned
}

pub(crate) fn layer_colors (bottom: (u8,u8,u8), top:(u8,u8,u8,u8)) -> (u8,u8,u8) {
    let top_share = top.3;
    let bottom_share = 255-top_share;
    (
        ((bottom.0 as u32 * bottom_share as u32 + top.0 as u32 * top_share as u32)>>8) as u8
        , ((bottom.1 as u32 * bottom_share as u32 + top.1 as u32 * top_share as u32)>>8) as u8
        , ((bottom.2 as u32 * bottom_share as u32 + top.2 as u32 * top_share as u32)>>8) as u8
    )
}

use std::cmp::*;
pub(crate) fn modify_color (color:(u8,u8,u8), brightness: f64, range:f64) -> (u8,u8,u8) {
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

pub(crate) fn is_in_filament(values: &ZoomerValuesScreen, pos: (i32, i32)) -> bool {

    let points = [
        pos
        , (pos.0, pos.1-1) // up
        , (pos.0, pos.1+1) // down
        , (pos.0-1, pos.1) // left
        , (pos.0+1, pos.1) // right
    ];

    let values = [
        get_escape_time(safe_sample(&values.values, points[0], values.res))
        ,get_escape_time(safe_sample(&values.values, points[1], values.res))
        ,get_escape_time(safe_sample(&values.values, points[2], values.res))
        ,get_escape_time(safe_sample(&values.values, points[3], values.res))
        ,get_escape_time(safe_sample(&values.values, points[4], values.res))
    ];

    slope_sign_changed(
        values[0], values[1], values[2], values[3], values[4]
    )
}


pub(crate) fn is_out_filament(values: &ZoomerValuesScreen, pos: (i32, i32)) -> bool {

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



pub(crate) fn is_node_tree(values: &ZoomerValuesScreen, pos: (i32, i32)) -> bool {

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

pub(crate) fn is_node(values: &ZoomerValuesScreen, pos: (i32, i32), thickness: u8) -> bool {

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

pub(crate) fn is_increased<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {
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

pub(crate) fn is_decreased<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {
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

pub(crate) fn is_changed<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {
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


pub(crate) fn slope_sign_changed<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {

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

pub(crate) fn is_local_minimum<T: PartialOrd > (value: Option<T>, up:Option<T>, down:Option<T>, left:Option<T>, right:Option<T>) -> bool {

    if let (Some(value), Some(up), Some(down), Some(left), Some(right)) = (&value, up, down, left, right) {
        if down > *value && *value < up
        && left > *value && *value < right {
            return true
        }
    }

    false
}

pub(crate) fn get_loop_period(value: Option<&ScreenValue>) -> Option<u32> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{..} => {return None}
            ScreenValue::Inside{loop_period, ..} => {
                return Some(*loop_period)
            }
        }
    } else {None}

}

pub(crate) fn get_escape_time(value: Option<&ScreenValue>) -> Option<u32> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{big_time, ..} => {return Some(*big_time)}
            ScreenValue::Inside{..} => {return None }
        }
    } else {None}

}

pub(crate) fn get_small_time(value: Option<&ScreenValue>) -> Option<u32> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{small_time, ..} => {return Some(*small_time)}
            ScreenValue::Inside{small_time, ..} => {return Some(*small_time)}
        }
    } else {None}

}

pub(crate) fn get_smallness(value: Option<&ScreenValue>) -> Option<f64> {

    if let Some(v) = value {
        match v {
            ScreenValue::Outside{smallness, ..} => {return Some(*smallness)}
            ScreenValue::Inside{smallness, ..} => {return Some(*smallness)}
        }
    } else {None}

}



use std::ops::*;
pub(crate) fn safe_sample<T: Index<usize, Output=J>, J>(stuff:&T, pos:(i32, i32), res:(u32, u32)) -> Option<&J> {
    if let Some(i) = index_from_pos_safe(&pos, res) {Some(&stuff[i])} else {None}
}
