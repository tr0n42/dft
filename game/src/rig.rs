//! Procedural legs for any walker: any number of legs, any number of joints
//! a leg.
//!
//! How it works (the usual way procedural walkers are built: feet planted on
//! the ground and stepped when they fall too far behind, each leg solved by
//! inverse kinematics):
//! - **Feet stay planted.** Each foot has a rest spot under the body. While
//!   the body moves, a planted foot stays where it is on the ground; once it
//!   is too far from its rest spot (the body has carried on), it steps.
//! - **Steps.** A step lifts the foot in an arc to a spot a little ahead of
//!   its rest spot (ahead by how fast the body is going), over a set time.
//! - **Gait groups.** Legs belong to groups that take turns: a leg may only
//!   lift while no leg of another group is in the air (a crab's diagonal
//!   pairs, a tripod's legs one at a time), so it never stands on too few.
//! - **On the real ground.** Where a foot comes down is found on the ground
//!   as drawn (the screen's copy of the world), not a flat line: feet stand
//!   on slopes, in hollows, on ledges.
//! - **The body on springs.** It rides at its height over the ground, sags
//!   while legs are in the air, jolts down as each foot lands, and leans
//!   toward its planted feet.
//! - **Each leg solved by FABRIK** (Aristidou and Lasenby's forward and
//!   backward reaching inverse kinematics): the chain of joints is pulled to
//!   the foot from the hip and back to the hip from the foot a few times,
//!   each bone keeping its length, starting from last frame's pose so it
//!   moves smoothly. Every leg has a bend: its joints are nudged toward a
//!   pole (up and outward) so knees always bow the same way.
//!
//! Positions are in the walker's own frame: u along the ground (+u
//! clockwise round the body), w up from where it stands.

use std::collections::HashMap;
use std::sync::Mutex;

/// One leg.
#[derive(Clone, Debug)]
pub struct LegSpec {
    /// Where it joins the body (u, w from the body's base).
    pub hip: (f32, f32),
    /// Its foot's rest spot on the ground (u from the body).
    pub rest: f32,
    /// Its bones, as shares of its whole length (any number).
    pub bones: Vec<f32>,
    /// Which gait group it steps with.
    pub group: u8,
    /// How strongly its joints bow toward its pole (up and out), 0 to 1.
    pub bend: f32,
}

/// A walker's legs and how it walks.
#[derive(Clone, Debug)]
pub struct RigSpec {
    pub legs: Vec<LegSpec>,
    /// The body's height over the ground (m).
    pub height: f32,
    /// Seconds a step takes, and how high a foot is lifted (m).
    pub step_s: f32,
    pub lift_m: f32,
    /// A foot steps once it is this far from its rest spot (m).
    pub trigger_m: f32,
    /// Each leg's whole length, as a share of the hip-to-rest distance.
    pub reach: f32,
    /// The body sags this much (m) for each leg in the air.
    pub sag_m: f32,
}

/// A walker's pose this frame.
pub struct Pose {
    /// The body's base over the ground (m).
    pub body: f32,
    /// Lean along the ground (m).
    pub lean: f32,
    /// Each leg's joints, hip first, foot last.
    pub legs: Vec<Vec<(f32, f32)>>,
    /// Feet that came down this frame.
    pub landed: Vec<bool>,
}

#[derive(Clone)]
struct Step {
    from: (f64, f32),
    to: (f64, f32),
    t: f32,
}

#[derive(Clone)]
struct Foot {
    /// Where it stands: metres round the body (counter-clockwise) and its
    /// height over the walker's own ground.
    s: f64,
    w: f32,
    step: Option<Step>,
}

struct State {
    last_t: f64,
    s: f64,
    vel: f64,
    feet: Vec<Foot>,
    joints: Vec<Vec<(f32, f32)>>,
    body: f32,
    vy: f32,
}

static STATES: Mutex<Option<HashMap<u64, State>>> = Mutex::new(None);

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// FABRIK: pull the chain `j` (hip first) so its last joint reaches
/// `target`, keeping each bone's length, its first joint fixed at `root`,
/// nudged toward `pole` so it bends the same way every frame.
pub fn fabrik(j: &mut [(f32, f32)], lens: &[f32], root: (f32, f32), target: (f32, f32), pole: (f32, f32), bend: f32) {
    let n = j.len();
    if n < 2 {
        return;
    }
    let total: f32 = lens.iter().sum();
    let to_target = dist(root, target);
    if to_target >= total * 0.999 {
        // Out of reach: straight toward it.
        let (dx, dy) = ((target.0 - root.0) / to_target.max(1e-6), (target.1 - root.1) / to_target.max(1e-6));
        j[0] = root;
        for i in 1..n {
            j[i] = (j[i - 1].0 + dx * lens[i - 1], j[i - 1].1 + dy * lens[i - 1]);
        }
        return;
    }
    for _ in 0..10 {
        // The bend: inner joints drawn toward the pole a little each pass.
        for joint in j.iter_mut().take(n - 1).skip(1) {
            joint.0 += (pole.0 - joint.0) * 0.08 * bend;
            joint.1 += (pole.1 - joint.1) * 0.08 * bend;
        }
        // Backward: from the foot.
        j[n - 1] = target;
        for i in (0..n - 1).rev() {
            let d = dist(j[i], j[i + 1]).max(1e-6);
            let k = lens[i] / d;
            j[i] = (j[i + 1].0 + (j[i].0 - j[i + 1].0) * k, j[i + 1].1 + (j[i].1 - j[i + 1].1) * k);
        }
        // Forward: from the hip.
        j[0] = root;
        for i in 0..n - 1 {
            let d = dist(j[i], j[i + 1]).max(1e-6);
            let k = lens[i] / d;
            j[i + 1] = (j[i].0 + (j[i + 1].0 - j[i].0) * k, j[i].1 + (j[i + 1].1 - j[i].1) * k);
        }
        if dist(j[n - 1], target) < 0.05 {
            break;
        }
    }
}

/// Step walker `id` (rig `spec`, standing at (x, y) in the world) to real
/// time `real_t`. `ground(s)` gives the ground's height (m) over the
/// walker's own ground at `s` metres round the body (None: none found
/// near). `moving` says whether it is walking; `airborne` tucks its legs in
/// (a leap).
pub fn step(id: u64, spec: &RigSpec, x: f64, y: f64, real_t: f64, airborne: bool, ground: &dyn Fn(f64) -> Option<f32>) -> Pose {
    let n = spec.legs.len();
    let r = (x * x + y * y).sqrt().max(1.0);
    let s_now = y.atan2(x) * r;
    // +u is clockwise: minus the distance round.
    let s_of = |u: f32| s_now - u as f64;
    let u_of = |s: f64| -(s - s_now) as f32;
    let mut all = STATES.lock().unwrap();
    let map = all.get_or_insert_with(Default::default);
    if map.len() > 128 {
        map.retain(|_, st| real_t - st.last_t < 30.0);
    }
    let rest_w = |s: f64| ground(s).unwrap_or(0.0);
    let fresh = || State {
        last_t: real_t,
        s: s_now,
        vel: 0.0,
        feet: spec.legs.iter().map(|l| Foot { s: s_of(l.rest), w: rest_w(s_of(l.rest)), step: None }).collect(),
        joints: Vec::new(),
        body: spec.height,
        vy: 0.0,
    };
    let st = map.entry(id).or_insert_with(fresh);
    if st.feet.len() != n {
        *st = fresh();
    }
    let dt = (real_t - st.last_t).clamp(0.0, 0.05) as f32;
    st.last_t = real_t;
    let ds = s_now - st.s;
    // A jump (a load, or out of sight for long): stand afresh.
    if ds.abs() > (spec.height as f64 * 3.0).max(60.0) {
        *st = fresh();
    }
    if dt > 0.0 {
        let v = (s_now - st.s) / dt as f64;
        st.vel += (v - st.vel) * (dt as f64 * 6.0).min(1.0);
    }
    st.s = s_now;
    let mut landed = vec![false; n];

    if airborne {
        // Feet drawn in under it; it stands afresh where it lands.
        for (i, l) in spec.legs.iter().enumerate() {
            st.feet[i] = Foot { s: s_of(l.rest * 0.3), w: -2.0, step: None };
        }
        let target = spec.height * 0.45;
        st.vy += ((target - st.body) * 7.0 - st.vy * 2.6) * dt;
        st.body += st.vy * dt;
    } else {
        // Feet in the air move on; those that land jolt the body.
        for (i, f) in st.feet.iter_mut().enumerate() {
            if let Some(sp) = &mut f.step {
                sp.t += dt / spec.step_s.max(0.05);
                if sp.t >= 1.0 {
                    f.s = sp.to.0;
                    f.w = sp.to.1;
                    f.step = None;
                    landed[i] = true;
                    st.vy -= 4.5;
                }
            } else {
                // A planted foot follows the ground under it (it may have
                // been eaten away).
                if let Some(g) = ground(f.s) {
                    f.w = g;
                }
            }
        }
        // Which legs want to step: too far from their rest spot.
        let lead = (st.vel * spec.step_s as f64 * 0.6).clamp(-spec.trigger_m as f64 * 1.5, spec.trigger_m as f64 * 1.5);
        let mut want: Vec<(f64, usize)> = Vec::new();
        for (i, l) in spec.legs.iter().enumerate() {
            let f = &st.feet[i];
            if f.step.is_some() {
                continue;
            }
            let off = (f.s - s_of(l.rest)).abs();
            if off > spec.trigger_m as f64 {
                want.push((off, i));
            }
        }
        // Only one group in the air at a time: the group already stepping,
        // else the group of the leg furthest behind.
        let busy: Option<u8> = spec.legs.iter().enumerate().find(|(i, _)| st.feet[*i].step.is_some()).map(|(_, l)| l.group);
        want.sort_by(|a, b| b.0.total_cmp(&a.0));
        let group = busy.or_else(|| want.first().map(|w| spec.legs[w.1].group));
        if let Some(g) = group {
            if busy.is_none() {
                // The whole group lifts together.
                for (i, l) in spec.legs.iter().enumerate() {
                    if l.group != g || st.feet[i].step.is_some() {
                        continue;
                    }
                    let to_s = s_of(l.rest) + lead;
                    let to_w = ground(to_s).unwrap_or(st.feet[i].w);
                    let from = (st.feet[i].s, st.feet[i].w);
                    st.feet[i].step = Some(Step { from, to: (to_s, to_w), t: 0.0 });
                }
            }
        }
        // The body: at its height, sagging for each leg in the air.
        let up = st.feet.iter().filter(|f| f.step.is_some()).count() as f32;
        let ground_here = ground(s_now).unwrap_or(0.0);
        let target = spec.height + ground_here.max(0.0) - up * spec.sag_m;
        st.vy += ((target - st.body) * 7.0 - st.vy * 2.6 - 30.0 * 0.04 * up) * dt;
        st.body += st.vy * dt;
    }

    // Where each foot is now, in the walker's frame.
    let mut feet_now = Vec::with_capacity(n);
    let mut lean = 0.0f32;
    for f in &st.feet {
        let p = match &f.step {
            Some(sp) => {
                let e = ease(sp.t) as f64;
                let s = sp.from.0 + (sp.to.0 - sp.from.0) * e;
                let w = sp.from.1 + (sp.to.1 - sp.from.1) * e as f32 + (sp.t * std::f32::consts::PI).sin() * spec.lift_m;
                (u_of(s), w)
            }
            None => {
                lean += u_of(f.s);
                (u_of(f.s), f.w)
            }
        };
        feet_now.push(p);
    }
    let lean = lean * 0.03;

    // Each leg by FABRIK, from last frame's pose.
    let body = st.body;
    let mut legs = Vec::with_capacity(n);
    if st.joints.len() != n {
        st.joints = vec![Vec::new(); n];
    }
    for (i, l) in spec.legs.iter().enumerate() {
        let root = (l.hip.0 + lean, body + l.hip.1);
        let foot = feet_now[i];
        let span = dist(root, (l.rest, 0.0)).max(1.0);
        let total = span * spec.reach;
        let lens: Vec<f32> = {
            let sum: f32 = l.bones.iter().sum::<f32>().max(1e-6);
            l.bones.iter().map(|b| b / sum * total).collect()
        };
        // The pole: up over the hip and out the way the leg reaches.
        let out = if l.rest >= l.hip.0 { 1.0 } else { -1.0 };
        let pole = (root.0 + out * total * 0.6, root.1 + total * 0.6);
        let j = &mut st.joints[i];
        if j.len() != lens.len() + 1 {
            // Start bowed toward the pole.
            *j = (0..=lens.len())
                .map(|k| {
                    let t = k as f32 / lens.len() as f32;
                    let bow = (t * std::f32::consts::PI).sin();
                    (root.0 + (foot.0 - root.0) * t + (pole.0 - root.0) * 0.3 * bow, root.1 + (foot.1 - root.1) * t + (pole.1 - root.1) * 0.3 * bow)
                })
                .collect();
        }
        fabrik(j, &lens, root, foot, pole, l.bend);
        legs.push(j.clone());
    }
    Pose { body, lean, legs, landed }
}
