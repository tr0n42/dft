//! The tripods' legs and gait (the forester and the abductor), as the swarm
//! mockup walks them (mockups/swarm-foundry.html, sceneTripod):
//!
//! - **A stride cycle, one leg at a time.** Three legs, leg i lifting at
//!   i/3 of the cycle, its swing taking 0.3 of it. The cycle runs on with the
//!   distance the body covers: a long, fast stride while it travels, a short
//!   one (a slow creep) while it works.
//! - **Feet stay put while planted** and swing in a low arc to a spot half a
//!   stride ahead of their rest spot. Where a foot comes down is found on the
//!   ground as drawn (the screen's copy of the world), not a flat line: feet
//!   stand on slopes, in hollows, on ledges. The only thing the mockup can't
//!   do.
//! - **The body hangs on its legs like a load on springs.** It sags when a
//!   leg lifts (two legs hold what three did), jolts down as each foot lands,
//!   surges while all three are planted and leans toward its planted feet.
//! - **Unfolding**: the legs spread from the package and the body heaves up,
//!   sags a little, holds. A leap tucks the legs in.
//!
//! The simulation decides where the body is; this only decides its legs and
//! how it rides on them. Positions are in the walker's own frame: u along the
//! ground (+u clockwise round the body), w up from where it stands.

use std::collections::HashMap;
use std::sync::Mutex;

/// The mockup's walking speed and stride period: a cycle carries the body
/// V * P metres at the working stride (times the stride while travelling).
const V: f32 = 2.3;
const P: f32 = 5.2;
/// The share of the cycle a leg is in the air.
const SWING: f32 = 0.3;
/// The longer stride while travelling.
const TRAVEL_STRIDE: f32 = 1.7;
/// The simulation's speed (m/s) over which it travels rather than works.
const TRAVEL_SPEED: f32 = 4.0;
/// Gravity (m/s/s), the mockup's.
const G: f32 = 30.0;

/// A tripod's legs, in the mockup's frame (x toward where it faces).
pub struct GaitSpec {
    /// Where each leg joins the body (x from the body's middle).
    pub hips: [f32; 3],
    /// Each foot's rest spot on the ground (x from the body's middle).
    pub spread: [f32; 3],
    /// The body's height over the ground (m).
    pub height: f32,
}

/// A tripod's pose this frame. Feet are in the walker's own frame (u, w),
/// in the mockup's leg order (rear leg first, as it faces).
pub struct Pose {
    /// The body's base over the ground (m).
    pub body: f32,
    /// The body's shift along the ground (u, m): its lean toward its planted
    /// feet and its surge.
    pub shift: f32,
    /// The lean alone (u, m), for what hangs off it.
    pub lean: f32,
    pub feet: [(f32, f32); 3],
    /// Feet that came down this frame.
    pub landed: [bool; 3],
    /// How far it has unfolded from its package (0 to 1; 1 once standing).
    pub unfold: f32,
}

#[derive(Clone, Copy)]
struct Leg {
    /// Where the foot stands: its direction (radians) and its height over
    /// the walker's own ground.
    a: f64,
    w: f32,
    /// A swing: from and to (direction, height), and how far through
    /// (0 to 1; negative: planted).
    from: (f64, f32),
    to: (f64, f32),
    swing: f32,
}

struct State {
    last_t: f64,
    /// The body's direction last frame.
    a: f64,
    gait: f32,
    stride: f32,
    /// Which way it is going along u (+1 or -1).
    dir: f32,
    /// The feet by side: slot 0 the furthest toward -u.
    legs: [Leg; 3],
    body: f32,
    vy: f32,
    /// The game time it began to unfold, and the state it was in.
    unfold_at: f64,
    state: u8,
}

static STATES: Mutex<Option<HashMap<u64, State>>> = Mutex::new(None);

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// An angle difference into (-pi, pi].
fn wrap(d: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let d = d.rem_euclid(t);
    if d > std::f64::consts::PI { d - t } else { d }
}

/// How far the body has surged ahead of its even pace (a share of the
/// stride) at `cyc` through the cycle: it goes at 1.3 while all three feet
/// are down and 0.6 while one swings (the mockup's), the same distance a
/// cycle all told.
fn surge(cyc: f32) -> f32 {
    let third = 1.0 / 3.0;
    let mean = (SWING * 0.6 + (third - SWING) * 1.3) / third;
    let p = cyc.rem_euclid(third);
    let slow = 0.6 / mean - 1.0;
    let fast = 1.3 / mean - 1.0;
    let at = if p < SWING { slow * p } else { slow * SWING + fast * (p - SWING) };
    // Centred on the even pace.
    at - slow * SWING * 0.5
}

/// Step tripod `id` (legs `spec`, standing at (x, y) in the world, facing
/// `face` along u) to real time `real_t`. `state` is the simulation's (1
/// unfolding, 4 leaping, the rest walking or working), `speed` its speed
/// (m/s of game time), `ground(u)` the ground's height (m) over the walker's
/// own ground `u` metres along (None: none found near).
#[allow(clippy::too_many_arguments)]
pub fn step(id: u64, spec: &GaitSpec, x: f64, y: f64, face: f32, now: f64, real_t: f64, state: u8, speed: f32, ground: &dyn Fn(f32) -> Option<f32>) -> Pose {
    let r = (x * x + y * y).sqrt().max(1.0);
    let a_now = y.atan2(x);
    // +u is clockwise: against the direction.
    let u_of = |a: f64| -(wrap(a - a_now) * r) as f32;
    let a_of = |u: f32| a_now - u as f64 / r;
    // Leg i of the mockup's frame stands in slot `slot(i)` (slots by side,
    // so a foot keeps its side when it turns about).
    let slot = |i: usize| if face > 0.0 { i } else { 2 - i };
    let rest = |i: usize| face * spec.spread[i];
    let mut all = STATES.lock().unwrap();
    let map = all.get_or_insert_with(Default::default);
    if map.len() > 64 {
        map.retain(|_, st| real_t - st.last_t < 30.0);
    }
    let fresh = || {
        let mut legs = [Leg { a: a_now, w: 0.0, from: (a_now, 0.0), to: (a_now, 0.0), swing: -1.0 }; 3];
        for i in 0..3 {
            let u = rest(i);
            let l = &mut legs[slot(i)];
            l.a = a_of(u);
            l.w = ground(u).unwrap_or(0.0);
        }
        State { last_t: real_t, a: a_now, gait: 0.0, stride: 1.0, dir: face, legs, body: spec.height, vy: 0.0, unfold_at: now, state }
    };
    let st = map.entry(id).or_insert_with(&fresh);
    // Out of sight a while: stand afresh.
    if real_t - st.last_t > 1.0 {
        *st = fresh();
    }
    let dt = (real_t - st.last_t).clamp(0.0, 0.05) as f32;
    st.last_t = real_t;
    // How far the body moved along u since last frame; a jump (a load, or
    // out of sight for long): stand afresh.
    let du = -(wrap(a_now - st.a) * r) as f32;
    if du.abs() > (spec.height * 3.0).max(60.0) {
        *st = fresh();
    }
    st.a = a_now;
    if state != st.state {
        if state == 1 {
            st.unfold_at = now;
        }
        st.state = state;
    }
    let mut landed = [false; 3];
    let mut unfold = 1.0;

    if state == 1 {
        // Unfolding: the mockup's 5.8 s from landing to standing, in the
        // simulation's 3 s. The legs spread from the package; standing is
        // work: it heaves up, sags a little, holds.
        let since = ((now - st.unfold_at) / 3.0 * 5.8) as f32;
        unfold = ((since - 0.6) / 2.6).clamp(0.0, 1.0);
        let stand = ((since - 3.4) / 2.4).clamp(0.0, 1.0);
        let k = ease(unfold);
        for i in 0..3 {
            let to = rest(i);
            let tw = ground(to).unwrap_or(0.0);
            let l = &mut st.legs[slot(i)];
            l.a = a_of(to * k);
            l.w = 1.0 + (tw - 1.0) * k + 6.0 * (k * std::f32::consts::PI).sin();
            l.swing = -1.0;
        }
        let sag = if stand > 0.7 { ((stand - 0.7) / 0.3 * std::f32::consts::PI).sin() * 2.5 } else { 0.0 };
        st.body = 4.0 + (spec.height - 3.0) * ease(stand) - sag;
        st.vy = 0.0;
    } else if state == 4 {
        // A leap: the feet drawn in under it; it stands afresh where it lands.
        for i in 0..3 {
            let l = &mut st.legs[slot(i)];
            let u = u_of(l.a);
            let k = (dt * 10.0).min(1.0);
            l.a = a_of(u + (rest(i) * 0.3 - u) * k);
            l.w += (-2.0 - l.w) * k;
            l.swing = -1.0;
        }
        st.vy += ((spec.height * 0.45 - st.body) * 7.0 - st.vy * 2.6) * dt;
        st.body += st.vy * dt;
    } else {
        // The stride: long and fast travelling, short working.
        let want = if speed > TRAVEL_SPEED { TRAVEL_STRIDE } else { 1.0 };
        st.stride += (want - st.stride) * (dt * 2.0).min(1.0);
        if du.abs() > 1e-4 {
            st.dir = du.signum();
        }
        let len = V * P * st.stride;
        let mut dg = du.abs() / len;
        // A foot in the air when the body stops still comes down.
        if st.legs.iter().any(|l| l.swing >= 0.0) {
            dg = dg.max(dt * 0.8);
        }
        st.gait = (st.gait + dg).rem_euclid(1000.0);
        let cyc = st.gait.fract();
        for i in 0..3 {
            let s0 = i as f32 / 3.0;
            let u = (cyc - s0).rem_euclid(1.0);
            let l = &mut st.legs[slot(i)];
            if u < SWING {
                if l.swing < 0.0 {
                    // Off it goes, to half a stride ahead of its rest spot.
                    l.from = (l.a, l.w);
                    let tu = rest(i) + st.dir * len * 0.5;
                    l.to = (a_of(tu), ground(tu).unwrap_or(l.w));
                }
                l.swing = u / SWING;
            } else if l.swing >= 0.0 {
                // The foot comes down under load: a jolt through the body.
                l.swing = -1.0;
                l.a = l.to.0;
                l.w = l.to.1;
                landed[i] = true;
                st.vy -= 4.5;
            } else if let Some(g) = ground(u_of(l.a)) {
                // A planted foot follows the ground under it (it may have
                // been eaten away).
                l.w = g;
            }
        }
        // The body on its springs: at its height, sagging for each leg in
        // the air.
        let up = st.legs.iter().filter(|l| l.swing >= 0.0).count() as f32;
        let target = spec.height + ground(0.0).unwrap_or(0.0).max(0.0) - up * 4.0;
        st.vy += ((target - st.body) * 7.0 - st.vy * 2.6 - G * 0.04 * up) * dt;
        st.body += st.vy * dt;
    }

    // Where each foot is now.
    let lift_m = 6.0 + 3.0 * ((st.stride - 1.0) / (TRAVEL_STRIDE - 1.0)).clamp(0.0, 1.0);
    let mut feet = [(0.0f32, 0.0f32); 3];
    let mut lean = 0.0;
    for (i, f) in feet.iter_mut().enumerate() {
        let l = &st.legs[slot(i)];
        *f = if l.swing >= 0.0 {
            let (fu, tu) = (u_of(l.from.0), u_of(l.to.0));
            if (tu - fu).abs() > 100.0 {
                (tu, l.to.1)
            } else {
                let e = ease(l.swing);
                (fu + (tu - fu) * e, l.from.1 + (l.to.1 - l.from.1) * e + (l.swing * std::f32::consts::PI).sin() * lift_m)
            }
        } else {
            let u = u_of(l.a);
            lean += u;
            (u, l.w)
        };
    }
    // (Unfolding, it stands square.)
    let lean = if state == 1 { 0.0 } else { lean * 0.03 };
    let surge_u = if state == 1 || state == 4 { 0.0 } else { st.dir * V * P * st.stride * surge(st.gait) };
    Pose { body: st.body, shift: lean + surge_u, lean, feet, landed, unfold }
}
