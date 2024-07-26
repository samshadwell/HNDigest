# frozen_string_literal: true

require 'aws-sdk-ses'

require_relative 'lib/digest_builder'
require_relative 'lib/digest_mailer'
require_relative 'lib/digest_renderer'
require_relative 'lib/post_snapshotter'
require_relative 'lib/storage_adapter'
require_relative 'lib/strategy_factory'
require_relative 'lib/strategies/over_point_threshold'
require_relative 'lib/strategies/top_n_posts'

# 5 AM UTC -> 10pm PDT, 9pm PST
SNAPSHOT_DAILY_HOUR = 5

def handle(*)
  current_time = Time.now
  date = Time.gm(
    current_time.year,
    current_time.month,
    current_time.day,
    SNAPSHOT_DAILY_HOUR
  )
  storage_adapter = StorageAdapter.new
  snapshotter = PostSnapshotter.new(storage_adapter:)
  all_posts = snapshotter.snapshot(date:).values

  digest_builder = DigestBuilder.new(storage_adapter:)
  mailer = DigestMailer.new(ses_client: Aws::SES::Client.new(region: 'us-west-2'))

  StrategyFactory.all_strategies.each do |strategy|
    posts = digest_builder.build_digest(
      digest_strategy: strategy,
      date:,
      posts: all_posts
    )
    renderer = DigestRenderer.new(posts:, date:)

    subscribers = storage_adapter.fetch_subscribers(type: strategy.type)
    next if subscribers.nil? || subscribers.empty?

    mailer.send_mail(renderer:, recipients: subscribers)
  end
end
