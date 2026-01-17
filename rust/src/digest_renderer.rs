use askama::Template;
use chrono::{DateTime, Utc};
use crate::types::Post;
use anyhow::{Result, Context};

#[derive(Template)]
#[template(path = "digest.html")]
struct DigestTemplate<'a> {
    posts: &'a [Post],
}

pub struct DigestRenderer<'a> {
    posts: &'a [Post],
    date: DateTime<Utc>,
}

impl<'a> DigestRenderer<'a> {
    pub fn new(posts: &'a [Post], date: DateTime<Utc>) -> Self {
        Self { posts, date }
    }

    pub fn subject(&self) -> String {
        format!("Hacker News Digest for {}", self.date.format("%b %-d, %Y"))
    }

    pub fn content(&self) -> Result<String> {
        let tmpl = DigestTemplate { posts: self.posts };
        tmpl.render().context("Failed to render digest template")
    }
}
