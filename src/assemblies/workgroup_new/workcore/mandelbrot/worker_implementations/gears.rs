//! Instantiates gears.wgsl for each stacked-i32 limb count.
// r[impl cz.seamless.gpu-preferred+1]

/// WGSL source for stacked arithmetic at `limbs` (1..=8).
pub fn gears_wgsl_for_limbs(limbs: u8) -> String {
    assert!((1..=8).contains(&limbs), "stacked GPU gears are limbs 1..=8");
    let n = limbs.to_string();
    include_str!("gears.wgsl").replace("{{LIMBS}}", &n)
}

/// Bout shader source: gears library + perturbation loop specialized to LIMBS.
/// Until the full stacked bout body lands, this still exports the arithmetic
/// library so pipelines can compile and orbit mirrors can cross-check mul/add.
pub fn stacked_bout_wgsl(limbs: u8) -> String {
    let mut src = gears_wgsl_for_limbs(limbs);
    src.push_str(
        r#"

// Smoke entry: proves the stacked arithmetic module compiles for this LIMBS.
@compute @workgroup_size(1)
fn gears_smoke_main(@builtin(global_invocation_id) _gid: vec3<u32>) {
    let a = sie_from_i32(3);
    let b = sie_from_i32(5);
    let _s = sie_add(a, b);
    let _p = sie_mul(a, b);
    let _c = sie_cmp(a, b);
}
"#,
    );
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
    fn stacked_bout_smoke_has_entry_point() {
        let src = stacked_bout_wgsl(4);
        assert!(src.contains("fn gears_smoke_main"));
        assert!(src.contains("sie_mul"));
    }
}
