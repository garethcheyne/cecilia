//! Playlists: the order of service.
//!
//! A playlist is a list of rows: tracks to play (each with its own delay,
//! verse plan and tempo) and text lines ("Sermon", "Notices"). Whoever runs
//! the service just presses Play/Next; the rows' delays decide whether the
//! next piece waits, starts by itself after a pause, or is cued at its first
//! note.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::player::VersePlan;
use crate::track::valid_id;

pub const PLAYLIST_SCHEMA: u32 = 1;

/// What happens after the previous row, before this track plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delay {
    /// Load it and wait for Play (the space bar).
    #[default]
    Wait,
    /// Start automatically this many seconds after the previous track ends.
    After { seconds: u32 },
    /// Load it positioned at its first note and wait, so Play starts the
    /// music at once.
    Cue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaylistRow {
    Track {
        track_id: String,
        #[serde(default)]
        delay: Delay,
        /// Verses to play; default every recorded verse.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verses: Option<VersePlan>,
        /// Percent; default 100.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tempo: Option<u16>,
        /// Free text shown beside the row (e.g. "Hymn 477, page 12").
        #[serde(default, skip_serializing_if = "String::is_empty")]
        note: String,
        /// The track's `modified` time when it was added or last checked,
        /// so the UI can warn if it was edited since.
        #[serde(default)]
        track_modified: u64,
    },
    /// A heading or reminder ("Sermon", "Collection").
    Text { text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Playlist {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub rows: Vec<PlaylistRow>,
    /// Start again from the top after the last row.
    #[serde(default)]
    pub repeat: bool,
    /// Play the track rows in a random order.
    #[serde(default)]
    pub shuffle: bool,
    /// Leave the stops as they are when a track ends (they're reset when the
    /// next one starts). Otherwise every stop is pushed in at the end.
    #[serde(default)]
    pub silent: bool,
    pub created: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub tracks: usize,
    pub modified: u64,
}

impl Playlist {
    pub fn summary(&self) -> PlaylistSummary {
        PlaylistSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            tracks: self.rows.iter().filter(|r| matches!(r, PlaylistRow::Track { .. })).count(),
            modified: self.modified,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema > PLAYLIST_SCHEMA {
            return Err("this playlist was saved by a newer version of Cecilia; update Cecilia to open it".into());
        }
        if self.schema != PLAYLIST_SCHEMA {
            return Err(format!("unsupported playlist schema {}", self.schema));
        }
        if !valid_id(&self.id) {
            return Err(format!("playlist id {:?} must be letters, digits, '-' or '_'", self.id));
        }
        if self.name.trim().is_empty() {
            return Err("a playlist needs a name".into());
        }
        for row in &self.rows {
            if let PlaylistRow::Track { track_id, tempo, verses, .. } = row {
                if !valid_id(track_id) {
                    return Err(format!("bad track id {track_id:?}"));
                }
                if tempo.is_some_and(|t| !(25..=400).contains(&t)) {
                    return Err("tempo must be 25-400%".into());
                }
                if let Some(VersePlan::Count(n)) = verses {
                    if *n == 0 || *n >= 20 {
                        return Err("verses must be 1-19".into());
                    }
                }
            }
        }
        Ok(())
    }

    /// Index of the first track row at or after `from`, in playing order.
    pub fn next_track(&self, order: &[usize], from: usize) -> Option<usize> {
        order
            .iter()
            .skip(from)
            .position(|&i| matches!(self.rows.get(i), Some(PlaylistRow::Track { .. })))
            .map(|p| p + from)
    }

    /// The playing order: row indexes, shuffled if asked (text rows stay put
    /// when not shuffling; when shuffling only tracks are played).
    pub fn order(&self, seed: u64) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.rows.len()).collect();
        if self.shuffle {
            order.retain(|&i| matches!(self.rows[i], PlaylistRow::Track { .. }));
            // Small deterministic shuffle (xorshift) so tests can rely on it.
            let mut x = seed | 1;
            for i in (1..order.len()).rev() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                order.swap(i, (x % (i as u64 + 1)) as usize);
            }
        }
        order
    }
}

/// Parse verses typed the way organists do, as in the original: a number
/// under 20 is a count ("4" = four verses); anything else is an explicit list
/// of verse numbers ("12357", or "1, 2, 3, 5, 7"; repeats allowed).
pub fn parse_verses(text: &str) -> Result<Option<VersePlan>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let bad = || format!("can't read the verses {t:?}");
    let list: Vec<usize> = if t.contains([',', ' ']) {
        t.split([',', ' ']).filter(|s| !s.is_empty()).map(|s| s.parse().map_err(|_| bad())).collect::<Result<_, _>>()?
    } else {
        let n: usize = t.parse().map_err(|_| bad())?;
        if n < 20 {
            return if n == 0 { Err(bad()) } else { Ok(Some(VersePlan::Count(n))) };
        }
        t.chars().map(|c| c.to_digit(10).map(|d| d as usize).ok_or_else(bad)).collect::<Result<_, _>>()?
    };
    if list.contains(&0) {
        return Err("verse numbers start at 1".into());
    }
    Ok(Some(VersePlan::List(list)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pl(rows: Vec<PlaylistRow>) -> Playlist {
        Playlist {
            schema: PLAYLIST_SCHEMA,
            id: "sunday".into(),
            name: "Sunday".into(),
            rows,
            repeat: false,
            shuffle: false,
            silent: false,
            created: 0,
            modified: 0,
        }
    }

    fn track(id: &str) -> PlaylistRow {
        PlaylistRow::Track {
            track_id: id.into(),
            delay: Delay::Wait,
            verses: None,
            tempo: None,
            note: String::new(),
            track_modified: 0,
        }
    }

    #[test]
    fn next_track_skips_text_rows() {
        let p = pl(vec![track("a"), PlaylistRow::Text { text: "Sermon".into() }, track("b")]);
        let order = p.order(1);
        assert_eq!(p.next_track(&order, 0), Some(0));
        assert_eq!(p.next_track(&order, 1), Some(2));
        assert_eq!(p.next_track(&order, 3), None);
    }

    #[test]
    fn shuffle_keeps_every_track_once() {
        let mut p = pl(vec![track("a"), PlaylistRow::Text { text: "x".into() }, track("b"), track("c")]);
        p.shuffle = true;
        let mut order = p.order(42);
        assert_eq!(order.len(), 3);
        order.sort();
        assert_eq!(order, [0, 2, 3]);
    }

    #[test]
    fn verse_lists_read_like_the_original() {
        assert_eq!(parse_verses(""), Ok(None));
        assert_eq!(parse_verses("4"), Ok(Some(VersePlan::Count(4))));
        assert_eq!(parse_verses("12357"), Ok(Some(VersePlan::List(vec![1, 2, 3, 5, 7]))));
        assert_eq!(parse_verses("1, 2, 4"), Ok(Some(VersePlan::List(vec![1, 2, 4]))));
        assert_eq!(parse_verses("1 1 2"), Ok(Some(VersePlan::List(vec![1, 1, 2]))));
        assert_eq!(parse_verses("12"), Ok(Some(VersePlan::Count(12))));
        assert!(parse_verses("1x").is_err());
        assert!(parse_verses("0").is_err());
    }

    #[test]
    fn validation() {
        let mut p = pl(vec![track("a")]);
        assert!(p.validate().is_ok());
        p.name = " ".into();
        assert!(p.validate().is_err());
        let mut p = pl(vec![PlaylistRow::Track {
            track_id: "a".into(),
            delay: Delay::After { seconds: 5 },
            verses: Some(VersePlan::Count(25)),
            tempo: None,
            note: String::new(),
            track_modified: 0,
        }]);
        assert!(p.validate().is_err());
        p.rows.clear();
        assert!(p.validate().is_ok());
    }
}
