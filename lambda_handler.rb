# frozen_string_literal: true

require_relative 'lib/digest_builder'
require_relative 'lib/post_snapshotter'
require_relative 'lib/storage_adapter'
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

  top_10_strategy = Strategies::TopNPosts.new(10)
  digest_builder = DigestBuilder.new(storage_adapter: storage_adapter)
  digest_builder.build_digest(digest_strategy: top_10_strategy, date: date)

  over_250_strategy = Strategies::OverPointThreshold.new(250)
  digest_builder.build_digest(digest_strategy: over_250_strategy, date: date)
end
