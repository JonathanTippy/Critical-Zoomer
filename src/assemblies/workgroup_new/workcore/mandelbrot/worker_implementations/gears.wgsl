//! Stacked i32 significand arithmetic for GPU gears (docs/design/tile_worker.md).
//! Instantiated per limb count 1..=8 by replacing `{{LIMBS}}` before create_shader_module.
// r[impl cz.seamless.gpu-preferred+1]

const LIMBS: u32 = {{LIMBS}}u;

struct Stacked {
    limbs: array<i32, {{LIMBS}}>
    , exp: i32
}

struct StackedC {
    re: Stacked
    , im: Stacked
}

struct StackedPair {
    a: Stacked
    , b: Stacked
}

fn uadd(a: u32, b: u32) -> vec2<u32> {
    // returns (sum, carry)
    let s = a + b;
    let c = select(0u, 1u, s < a);
    return vec2<u32>(s, c);
}

fn sie_is_zero(a: Stacked) -> bool {
    for (var i = 0u; i < LIMBS; i = i + 1u) {
        if a.limbs[i] != 0 {
            return false;
        }
    }
    return true;
}

fn sie_is_neg(a: Stacked) -> bool {
    return a.limbs[LIMBS - 1u] < 0;
}

fn sie_zero() -> Stacked {
    var out: Stacked;
    for (var i = 0u; i < LIMBS; i = i + 1u) {
        out.limbs[i] = 0;
    }
    out.exp = 0;
    return out;
}

fn sie_from_i32(v: i32) -> Stacked {
    var out: Stacked;
    out.limbs[0] = v;
    let fill = select(0, -1, v < 0);
    for (var i = 1u; i < LIMBS; i = i + 1u) {
        out.limbs[i] = fill;
    }
    out.exp = 0;
    return out;
}

fn sie_neg(a: Stacked) -> Stacked {
    var out: Stacked;
    var carry = 1u;
    for (var i = 0u; i < LIMBS; i = i + 1u) {
        let not_x = ~bitcast<u32>(a.limbs[i]);
        let sc = uadd(not_x, carry);
        out.limbs[i] = bitcast<i32>(sc.x);
        carry = sc.y;
    }
    out.exp = a.exp;
    return out;
}

fn sie_abs(a: Stacked) -> Stacked {
    if sie_is_neg(a) {
        return sie_neg(a);
    }
    return a;
}

fn sie_shl_bits(a: Stacked, bits: u32) -> Stacked {
    var out = a;
    if bits == 0u || sie_is_zero(a) {
        return out;
    }
    var remaining = bits;
    while remaining > 0u {
        let step = min(remaining, 31u);
        var carry = 0u;
        for (var i = 0u; i < LIMBS; i = i + 1u) {
            let cur = bitcast<u32>(out.limbs[i]);
            let shifted = (cur << step) | carry;
            carry = cur >> (32u - step);
            out.limbs[i] = bitcast<i32>(shifted);
        }
        remaining = remaining - step;
    }
    return out;
}

fn sie_align(a: Stacked, b: Stacked) -> StackedPair {
    var aa = a;
    var bb = b;
    if aa.exp == bb.exp {
        return StackedPair(aa, bb);
    }
    if aa.exp > bb.exp {
        aa = sie_shl_bits(aa, u32(aa.exp - bb.exp));
        aa.exp = bb.exp;
    } else {
        bb = sie_shl_bits(bb, u32(bb.exp - aa.exp));
        bb.exp = aa.exp;
    }
    return StackedPair(aa, bb);
}

fn sie_add(a: Stacked, b: Stacked) -> Stacked {
    let ab = sie_align(a, b);
    var out: Stacked;
    var carry = 0u;
    for (var i = 0u; i < LIMBS; i = i + 1u) {
        let av = bitcast<u32>(ab.a.limbs[i]);
        let bv = bitcast<u32>(ab.b.limbs[i]);
        let s0 = uadd(av, carry);
        let s1 = uadd(s0.x, bv);
        out.limbs[i] = bitcast<i32>(s1.x);
        carry = s0.y + s1.y;
    }
    out.exp = ab.a.exp;
    return out;
}

fn sie_sub(a: Stacked, b: Stacked) -> Stacked {
    var nb = sie_neg(b);
    nb.exp = b.exp;
    return sie_add(a, nb);
}

fn umul32(a: u32, b: u32) -> vec2<u32> {
    // 32x32 -> (lo, hi)
    let a_lo = a & 0xffffu;
    let a_hi = a >> 16u;
    let b_lo = b & 0xffffu;
    let b_hi = b >> 16u;
    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;
    var mid = (p0 >> 16u) + (p1 & 0xffffu) + (p2 & 0xffffu);
    let lo = (p0 & 0xffffu) | ((mid & 0xffffu) << 16u);
    mid = mid >> 16u;
    let hi = p3 + (p1 >> 16u) + (p2 >> 16u) + mid;
    return vec2<u32>(lo, hi);
}

fn sie_mul(a: Stacked, b: Stacked) -> Stacked {
    if sie_is_zero(a) || sie_is_zero(b) {
        return sie_zero();
    }
    let an = sie_is_neg(a);
    let bn = sie_is_neg(b);
    let am = sie_abs(a);
    let bm = sie_abs(b);
    var wide: array<u32, 16>;
    for (var i = 0u; i < 16u; i = i + 1u) {
        wide[i] = 0u;
    }
    for (var i = 0u; i < LIMBS; i = i + 1u) {
        let ai = bitcast<u32>(am.limbs[i]);
        for (var j = 0u; j < LIMBS; j = j + 1u) {
            let prod = umul32(ai, bitcast<u32>(bm.limbs[j]));
            let idx = i + j;
            let s0 = uadd(wide[idx], prod.x);
            wide[idx] = s0.x;
            let s1 = uadd(wide[idx + 1u], prod.y);
            let s2 = uadd(s1.x, s0.y);
            wide[idx + 1u] = s2.x;
            let carry2 = s1.y + s2.y;
            if carry2 != 0u && idx + 2u < 16u {
                wide[idx + 2u] = wide[idx + 2u] + carry2;
            }
        }
    }
    // Propagate any residual carries in the wide product.
    var carry = 0u;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let s = uadd(wide[i], carry);
        wide[i] = s.x;
        carry = s.y;
    }
    var out: Stacked;
    for (var i = 0u; i < LIMBS; i = i + 1u) {
        out.limbs[i] = bitcast<i32>(wide[i]);
    }
    out.exp = am.exp + bm.exp;
    if an != bn {
        let e = out.exp;
        out = sie_neg(out);
        out.exp = e;
    }
    return out;
}

fn sie_cmp(a: Stacked, b: Stacked) -> i32 {
    let ab = sie_align(a, b);
    let d = sie_sub(ab.a, ab.b);
    if sie_is_zero(d) {
        return 0;
    }
    if sie_is_neg(d) {
        return -1;
    }
    return 1;
}

fn sie_norm2(z: StackedC) -> Stacked {
    return sie_add(sie_mul(z.re, z.re), sie_mul(z.im, z.im));
}

fn sie_c_add(a: StackedC, b: StackedC) -> StackedC {
    return StackedC(sie_add(a.re, b.re), sie_add(a.im, b.im));
}

fn sie_c_mul(a: StackedC, b: StackedC) -> StackedC {
    return StackedC(
        sie_sub(sie_mul(a.re, b.re), sie_mul(a.im, b.im))
        , sie_add(sie_mul(a.re, b.im), sie_mul(a.im, b.re))
    );
}

fn sie_c_scale2(z: StackedC) -> StackedC {
    return StackedC(sie_add(z.re, z.re), sie_add(z.im, z.im));
}
