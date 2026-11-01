//! First steps: a handful of nudges for brand-new arrivals. One hint at a
//! time, each cleared by doing the thing it suggests.

pub const DONE: u8 = 99;

/// Something a player just did that a step might be waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Walk,
    Say,
    Make,
    Plant,
    Who,
}

pub struct Step {
    pub act: Act,
    pub hint: &'static str,
    /// What tapping the hint (or its button) does.
    pub cmd: &'static str,
    /// Said when the step is done, before the next hint.
    pub cheer: &'static str,
}

pub const STEPS: &[Step] = &[
    Step {
        act: Act::Walk,
        hint: "✨ Try walking: type n to go north (or tap ↑)",
        cmd: "n",
        cheer: "🚶 Nice walking!",
    },
    Step {
        act: Act::Say,
        hint: "🌼 Say hello: type 'hi",
        cmd: "'hi",
        cheer: "💬 Everyone nearby heard you!",
    },
    Step {
        act: Act::Make,
        hint: "🪑 Make something: make a little bench",
        cmd: "make a little bench",
        cheer: "✨ You made a thing! It'll stay here for everyone to see.",
    },
    Step {
        act: Act::Plant,
        hint: "🌸 Grow something: plant flowers",
        cmd: "plant flowers",
        cheer: "🌸 Gorgeous.",
    },
    Step {
        act: Act::Who,
        hint: "👀 Last one: type who to see who's out in the world",
        cmd: "who",
        cheer: "🎉 You're a natural! Talk to anyone, anywhere with @Name hi. Type help any time to see everything else.",
    },
];

pub fn step(n: u8) -> Option<&'static Step> {
    STEPS.get(n as usize)
}
