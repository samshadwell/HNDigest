use crate::configuration::{POINT_THRESHOLD_VALUES, TOP_N_VALUES};
use crate::types::Post;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DigestStrategy {
    TopN(usize),
    OverPointThreshold(i32),
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

impl DigestStrategy {
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
}
