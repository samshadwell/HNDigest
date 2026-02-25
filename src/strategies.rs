use crate::types::Post;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

const TOP_N_VALUES: &[usize] = &[10, 20, 50];
const POINT_THRESHOLD_VALUES: &[i32] = &[500, 250, 100];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DigestStrategy {
    TopN(usize),
    OverPointThreshold(i32),
}

impl DigestStrategy {
    /// Returns all configured digest strategies.
    pub fn all() -> Vec<DigestStrategy> {
        TOP_N_VALUES
            .iter()
            .map(|&n| DigestStrategy::TopN(n))
            .chain(
                POINT_THRESHOLD_VALUES
                    .iter()
                    .map(|&t| DigestStrategy::OverPointThreshold(t)),
            )
            .collect()
    }

    /// Returns the maximum TopN value across all strategies.
    pub fn max_top_n() -> usize {
        TOP_N_VALUES.iter().copied().max().unwrap_or(50)
    }

    /// Returns the minimum point threshold across all strategies.
    pub fn min_point_threshold() -> i32 {
        POINT_THRESHOLD_VALUES.iter().copied().min().unwrap_or(100)
    }

    pub fn select(&self, posts: &[Post]) -> Vec<Post> {
        match self {
            Self::TopN(n) => posts.iter().take(*n).cloned().collect(),
            Self::OverPointThreshold(threshold) => posts
                .iter()
                .filter(|p| p.points >= *threshold)
                .cloned()
                .collect(),
        }
    }

    /// Get a human-readable description of this digest strategy.
    pub fn description(&self) -> String {
        match self {
            Self::TopN(n) => format!("Top {} stories by points", n),
            Self::OverPointThreshold(t) => format!("All stories with {}+ points", t),
        }
    }
}

impl FromStr for DigestStrategy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(n) = s.strip_prefix("TOP_N#") {
            let n: usize = n.parse()?;
            if !TOP_N_VALUES.contains(&n) {
                anyhow::bail!(
                    "Invalid TOP_N value: {}. Valid values are: {:?}",
                    n,
                    TOP_N_VALUES
                );
            }
            Ok(DigestStrategy::TopN(n))
        } else if let Some(threshold) = s.strip_prefix("POINT_THRESHOLD#") {
            let threshold: i32 = threshold.parse()?;
            if !POINT_THRESHOLD_VALUES.contains(&threshold) {
                anyhow::bail!(
                    "Invalid POINT_THRESHOLD value: {}. Valid values are: {:?}",
                    threshold,
                    POINT_THRESHOLD_VALUES
                );
            }
            Ok(DigestStrategy::OverPointThreshold(threshold))
        } else {
            anyhow::bail!("Invalid strategy format: {}", s)
        }
    }
}

impl Serialize for DigestStrategy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DigestStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DigestStrategy::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for DigestStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TopN(n) => write!(f, "TOP_N#{}", n),
            Self::OverPointThreshold(threshold) => write!(f, "POINT_THRESHOLD#{}", threshold),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_n_roundtrip() {
        for &n in TOP_N_VALUES {
            let strategy = DigestStrategy::TopN(n);
            let serialized = strategy.to_string();
            let deserialized: DigestStrategy = serialized.parse().unwrap();
            assert_eq!(strategy, deserialized);
        }
    }

    #[test]
    fn test_point_threshold_roundtrip() {
        for &threshold in POINT_THRESHOLD_VALUES {
            let strategy = DigestStrategy::OverPointThreshold(threshold);
            let serialized = strategy.to_string();
            let deserialized: DigestStrategy = serialized.parse().unwrap();
            assert_eq!(strategy, deserialized);
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        for &n in TOP_N_VALUES {
            let strategy = DigestStrategy::TopN(n);
            let json = serde_json::to_string(&strategy).unwrap();
            let deserialized: DigestStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, deserialized);
        }

        for &threshold in POINT_THRESHOLD_VALUES {
            let strategy = DigestStrategy::OverPointThreshold(threshold);
            let json = serde_json::to_string(&strategy).unwrap();
            let deserialized: DigestStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, deserialized);
        }
    }

    #[test]
    fn test_invalid_top_n_value() {
        let result: Result<DigestStrategy, _> = "TOP_N#999".parse();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid TOP_N value"));
    }

    #[test]
    fn test_invalid_point_threshold_value() {
        let result: Result<DigestStrategy, _> = "POINT_THRESHOLD#999".parse();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid POINT_THRESHOLD value"));
    }

    #[test]
    fn test_invalid_format() {
        let result: Result<DigestStrategy, _> = "INVALID#10".parse();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid strategy format"));
    }

    #[test]
    fn test_display_format() {
        assert_eq!(DigestStrategy::TopN(10).to_string(), "TOP_N#10");
        assert_eq!(
            DigestStrategy::OverPointThreshold(500).to_string(),
            "POINT_THRESHOLD#500"
        );
    }

    #[test]
    fn test_all_strategies() {
        let all = DigestStrategy::all();
        assert_eq!(all.len(), TOP_N_VALUES.len() + POINT_THRESHOLD_VALUES.len());

        for &n in TOP_N_VALUES {
            assert!(all.contains(&DigestStrategy::TopN(n)));
        }
        for &t in POINT_THRESHOLD_VALUES {
            assert!(all.contains(&DigestStrategy::OverPointThreshold(t)));
        }
    }

    #[test]
    fn test_max_top_n() {
        assert_eq!(DigestStrategy::max_top_n(), 50);
    }

    #[test]
    fn test_min_point_threshold() {
        assert_eq!(DigestStrategy::min_point_threshold(), 100);
    }

    fn make_post(id: &str, points: i32) -> Post {
        Post {
            object_id: id.to_string(),
            title: format!("Post {}", id),
            url: None,
            points,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn top_n_select_returns_at_most_n_posts() {
        let posts: Vec<Post> = (0..20)
            .map(|i| make_post(&i.to_string(), i as i32))
            .collect();
        let selected = DigestStrategy::TopN(5).select(&posts);
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn top_n_select_fewer_than_n_returns_all() {
        let posts = vec![make_post("a", 100), make_post("b", 200)];
        let selected = DigestStrategy::TopN(10).select(&posts);
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn top_n_select_preserves_input_order() {
        // select() is a take(), so it keeps whatever order was passed in;
        // callers are responsible for sorting beforehand.
        let posts = vec![
            make_post("a", 300),
            make_post("b", 100),
            make_post("c", 200),
        ];
        let selected = DigestStrategy::TopN(2).select(&posts);
        assert_eq!(selected[0].object_id, "a");
        assert_eq!(selected[1].object_id, "b");
    }

    #[test]
    fn over_point_threshold_filters_below_threshold() {
        let posts = vec![make_post("a", 500), make_post("b", 250), make_post("c", 99)];
        let selected = DigestStrategy::OverPointThreshold(250).select(&posts);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|p| p.points >= 250));
    }

    #[test]
    fn over_point_threshold_includes_exact_threshold() {
        let posts = vec![make_post("a", 100)];
        let selected = DigestStrategy::OverPointThreshold(100).select(&posts);
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn over_point_threshold_empty_when_none_qualify() {
        let posts = vec![make_post("a", 50)];
        let selected = DigestStrategy::OverPointThreshold(100).select(&posts);
        assert!(selected.is_empty());
    }

    #[test]
    fn description_is_human_readable() {
        assert!(DigestStrategy::TopN(10).description().contains("10"));
        assert!(
            DigestStrategy::OverPointThreshold(500)
                .description()
                .contains("500")
        );
    }
}
