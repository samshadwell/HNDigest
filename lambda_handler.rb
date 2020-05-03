# frozen_string_literal: true

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
  snapshotter = PostSnapshotter.new(storage_adapter: storage_adapter)
  snapshotter.snapshot(date: date)

  digest_builder = DigestBuilder.new(storage_adapter: storage_adapter)
  mailer = DigestMailer.new(api_key: ENV['SENDGRID_API_KEY'])

  StrategyFactory.all_strategies.each do |strategy|
    posts = digest_builder.build_digest(digest_strategy: strategy, date: date)
    renderer = DigestRenderer.new(posts: posts, date: date)

    subscribers = storage_adapter.fetch_subscribers(type: strategy.type) || {}
    recipients = subscribers['emails'] || []
    next if recipients.empty?

    mailer.send_mail(renderer: renderer, recipients: recipients)
  end
end
