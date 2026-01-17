use crate::types::Post;

pub trait DigestStrategy: Sync + Send {
    fn type_(&self) -> String;
    fn select(&self, posts: &[Post]) -> Vec<Post>;
}

pub struct TopNPosts {
    pub n: usize,
}

impl DigestStrategy for TopNPosts {
    fn type_(&self) -> String {
        format!("TOP_N#{}", self.n)
    }

    fn select(&self, posts: &[Post]) -> Vec<Post> {
        posts.iter().take(self.n).cloned().collect()
    }
}

pub struct OverPointThreshold {
    pub threshold: i32,
}

impl DigestStrategy for OverPointThreshold {
    fn type_(&self) -> String {
        format!("POINT_THRESHOLD#{}", self.threshold)
    }

    fn select(&self, posts: &[Post]) -> Vec<Post> {
        posts
            .iter()
            .filter(|p| p.points >= self.threshold)
            .cloned()
            .collect()
    }
}
