//! What a spot sounds like: rain and thunder from the cloud overhead, wind
//! that's stronger on open ground, waves by the sea, birds in the woods by
//! day, crickets in the grass at night, and the crackle of a campfire. It
//! goes out in the side panel as levels from 0 to 1, and the browser makes
//! every sound itself from noise and little oscillators (see "Sounds of the
//! island" in web/index.html). Indoors, the rain drums on the roof and the
//! wind is only just there.
//!
//! Levels come in quarters, so the side panel only goes out again when
//! something you'd hear has really changed.

use super::Game;
use crate::protocol::Sounds;
use crate::server::terrain::Terrain as T;
use crate::server::weather::{Clock, Phase, STORMY, Season};
use crate::server::world::{DIRS, room_of};

/// To the nearest quarter, so small wobbles don't send a fresh panel.
fn quarter(v: f64) -> f32 {
    ((v.clamp(0.0, 1.0) * 4.0).round() / 4.0) as f32
}

impl Game {
    pub(super) fn sounds_at(&self, x: i64, y: i64) -> Sounds {
        let indoors = room_of(x, y).is_some();
        let (ox, oy) = self.world.outside(x, y);
        let wet = self.world.weather.wetness(ox, oy);
        let winter = Clock::season() == Season::Winter;
        let phase = Clock::phase();
        let ground = self.world.terrain(ox, oy);
        let around: Vec<T> = DIRS
            .iter()
            .map(|d| self.world.terrain(ox + d.2, oy + d.3))
            .collect();
        let near = |t: T| ground == t || around.contains(&t);
        // Snow falls silently, and rooms keep most of the weather out.
        let roof: f64 = if indoors { 0.35 } else { 1.0 };

        let rain = if winter || wet <= 0.3 {
            0.0
        } else if wet > 0.55 {
            1.0
        } else {
            0.4
        };
        let thunder = if !winter && wet > STORMY {
            roof.max(0.5)
        } else {
            0.0
        };

        let gusty = (self.world.weather.speed() - 0.15) / 0.35;
        let open = if indoors {
            0.15
        } else {
            match ground {
                T::Hill | T::Stone | T::Sand | T::Water | T::DeepWater => 1.0,
                T::Forest => 0.45,
                _ => 0.7,
            }
        };

        let sea = |t: &T| matches!(t, T::Water | T::DeepWater);
        let waves = if indoors {
            0.0
        } else if sea(&ground) || around.iter().any(sea) {
            1.0
        } else if (-2..=2)
            .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
            .any(|(dx, dy)| sea(&self.world.terrain(ox + dx, oy + dy)))
        {
            0.5
        } else {
            0.0
        };

        let daytime = matches!(phase, Phase::Dawn | Phase::Day);
        let birds = if indoors || !daytime || wet > 0.55 {
            0.0
        } else {
            let b = match ground {
                T::Forest => 1.0,
                T::Meadow => 0.5,
                _ if near(T::Forest) => 0.5,
                T::Grass | T::Hill => 0.25,
                _ => 0.0,
            };
            if winter { b * 0.5 } else { b }
        };

        let evening = matches!(phase, Phase::Dusk | Phase::Night);
        let crickets = if indoors || !evening || winter || wet > 0.3 {
            0.0
        } else {
            match ground {
                T::Grass | T::Meadow => 1.0,
                T::Forest | T::Hill | T::Path => 0.5,
                _ => 0.0,
            }
        };

        // A campfire right here crackles; one a step away, a little.
        let fire = self
            .world
            .things
            .iter()
            .filter(|t| super::cook::kitchen(&t.name) == Some(super::cook::FIRE))
            .map(|t| (t.x - x).abs().max((t.y - y).abs()))
            .min()
            .map_or(0.0, |d| match d {
                0 => 1.0,
                1 if !indoors => 0.5,
                _ => 0.0,
            });

        Sounds {
            rain: quarter(rain * roof),
            thunder: quarter(thunder),
            wind: quarter(gusty * open),
            waves: quarter(waves),
            birds: quarter(birds),
            crickets: quarter(crickets),
            fire: quarter(fire),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::quarter;

    #[test]
    fn levels_come_in_quarters() {
        assert_eq!(quarter(0.1), 0.0);
        assert_eq!(quarter(0.3), 0.25);
        assert_eq!(quarter(0.62), 0.5);
        assert_eq!(quarter(2.0), 1.0);
        assert_eq!(quarter(-1.0), 0.0);
    }
}
