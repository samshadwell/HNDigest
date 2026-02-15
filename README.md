# Hacker Digest

Hosted at [hndigest.samshadwell.com/](https://hndigest.samshadwell.com/), Hacker Digest is a daily email digest of the highest-voted [Hacker News](https://news.ycombinator.com/) submissions. The newsletter is free and publicly availalbe, so please sign up!

I created this project for two reasons:
1. To help me stay up-to-date with the latest tech news without having to prowl the Hacker News site periodically
2. As a low-stakes project to experiment with various technologies

It's deployed on an AWS serverless stack (Lambda, SES, S3, DynamoDB, CloudFront), with infrastructure configured via OpenTofu (see the `infrastructure` directory). The logic itself is implemented in Rust, and relies heavily on [Algolia's publicly-available Hacker News search API](https://hn.algolia.com/api). The primary design constraints for this project are low-cost and simplicity.

If you have any suggestions or contributions, please open an issue or pull request. I'm extremely open to making any changes that might make this project more useful to you. You can also send me email at [hi@samshadwell.com](mailto:hi@samshadwell.com) if you have any questions or feedback that don't make sense for GitHub.

I have no plans to monetize this project in any way (ads or otherwise). It currently costs me $0.02 per month to run for myself and a small handful of friends. I'm not accepting donations for this work. If you really want to give me money, that's too bad!
