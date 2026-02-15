# Hacker Digest

Hosted at [hndigest.samshadwell.com](https://hndigest.samshadwell.com/), Hacker Digest is a daily email digest of the highest-voted [Hacker News](https://news.ycombinator.com/) submissions. The newsletter is free and publicly available, so please sign up!

I created this project for two reasons:
1. To help me stay up-to-date with the latest tech news without having to prowl the Hacker News site
2. As a low-stakes project to experiment with various technologies

It's deployed on an AWS serverless stack (Lambda, SES, S3, DynamoDB, CloudFront), with infrastructure configured via OpenTofu (see the `infrastructure` directory). The logic itself is implemented in Rust, and relies heavily on [Algolia's publicly-available Hacker News search API](https://hn.algolia.com/api). The primary design constraints for this project are low-cost and simplicity.

If you have any suggestions or contributions, please open an issue or pull request. I'm extremely open to making any changes that might make this project more useful to you. You can also send me email at [hi@samshadwell.com](mailto:hi@samshadwell.com) if you have any questions or feedback that don't make sense for GitHub.

## Setup

This repo uses [mise](https://mise.jdx.dev/) to manage development dependencies. You can install it by following their [getting started guide](https://mise.jdx.dev/getting-started.html). Once mise is installed, you can run `mise install` from the repo root to install all the necessary tools. See [mise.toml](mise.toml) for what this will include.

## Building

Code can be compiled by running `cargo build` from the repo root. Because it is deployed in a Lambda, `cargo run` won't do anything useful locally.

## Deployment

If you want to self-host this project, I provide all the OpenTofu configuration needed for my own deployment. I've attempted to make them modular enough to be useful for self-hosting logical pieces of this project. The general flow is:
1. Bootstrap the project with `tofu init && tofu apply` `infrastructure/bootstrap` directory. This will create an S3 bucket for OpenTofu remote state and a Github OIDC connector for continuous deployment.
2. Modify the environments to suit your needs (see `infrastructure/environments`)
3. Run the same `tofu init && tofu apply` from the modified environment directory

This has some known limitations:
- You will likely get some branding that references me or my domain or identity
- Your newsletters will include links to unsubscribe, which won't work unless you use the `web` module

If running without web you'll need to add subscribers via CLI, like this:

```
$ DYNAMODB_TABLE=<your_table_name> cargo run --bin add-subscriber you@example.com <strategy>
```

Where `<strategy>` is one of `TOP_N#<n>` or `POINT_THRESHOLD#<threshold>` (e.g, `TOP_N#10`). I personally use `POINT_THRESHOLD#250`.

## Money

I have no plans to monetize this project in any way (ads, sponsorships, selling the email recipients on the dark web, etc.). It currently costs me $0.02 per month to run for myself and a small handful of subscribers, which AWS graciously rounds down to 0.
