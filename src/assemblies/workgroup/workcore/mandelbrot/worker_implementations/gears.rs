//! Instantiates gears.wgsl for each stacked-i32 limb count.
// r[impl cz.seamless.gpu-preferred+1]

/// WGSL source for stacked arithmetic at `limbs` (1..=8).
pub fn gears_wgsl_for_limbs(limbs: u8) -> String {
    assert!((1..=8).contains(&limbs), "stacked GPU gears are limbs 1..=8");
    let n = limbs.to_string();
    include_str!("gears.wgsl").replace("{{LIMBS}}", &n)
}

/// Full stacked bout: gears library + perturbation loop specialized to LIMBS.
/// Uses the same GpuPertPoint / orbit bindings as the f32 bout; iteration for
/// zero-orbit seats runs in stacked i32 so StackedI32 gears stay GPU-native.
pub fn stacked_bout_wgsl(limbs: u8) -> String {
    let mut src = gears_wgsl_for_limbs(limbs);
    src.push_str(include_str!("stacked_bout_tail.wgsl"));
    src
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiates_all_eight_limb_counts() {
        for limbs in 1u8..=8 {
            let src = gears_wgsl_for_limbs(limbs);
            assert!(src.contains(&format!("const LIMBS: u32 = {limbs}u;")));
            assert!(src.contains(&format!("array<i32, {limbs}>")));
            assert!(!src.contains("{{LIMBS}}"));
        }
    }

    #[test]
    fn stacked_bout_has_main_entry() {
        let src = stacked_bout_wgsl(4);
        assert!(src.contains("fn main"));
        assert!(src.contains("sie_mul"));
        assert!(src.contains("GpuPertPoint"));
    }
}
