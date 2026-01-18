use crate::types::Post;
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum DigestStrategy {
    TopN(usize),
    OverPointThreshold(i32),
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
